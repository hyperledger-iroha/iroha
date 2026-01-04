//! This module contains data and structures related only to smart contract execution

pub mod payloads {
    //! Contexts with function arguments for different entrypoints

    use norito::codec::{Decode, Encode};

    use crate::{block::BlockHeader, prelude::*};

    /// Context for smart contract entrypoint
    #[derive(Debug, Clone, Encode, Decode)]
    pub struct SmartContractContext {
        /// Account that submitted the transaction containing the smart contract
        pub authority: AccountId,
        /// Block currently being processed
        pub curr_block: BlockHeader,
    }

    /// Context for trigger entrypoint
    #[derive(Encode, Decode)]
    #[cfg_attr(not(feature = "fast_dsl"), derive(Debug, Clone))]
    pub struct TriggerContext {
        /// Id of this trigger
        pub id: TriggerId,
        /// Account that registered the trigger
        pub authority: AccountId,
        /// Block currently being processed
        pub curr_block: BlockHeader,
        /// Event which triggered the execution
        pub event: EventBox,
    }

    /// Context for migrate entrypoint
    #[derive(Debug, Clone, Encode, Decode)]
    pub struct ExecutorContext {
        /// Account that is executing the operation
        pub authority: AccountId,
        /// Block currently being processed (or latest block hash for queries)
        pub curr_block: BlockHeader,
    }

    /// Generic payload for `validate_*()` entrypoints of executor.
    #[derive(Debug, Clone, Encode, Decode)]
    pub struct Validate<T> {
        /// Context of the executor
        pub context: ExecutorContext,
        /// Operation to be validated
        pub target: T,
    }
}

// Smart contract manifest types and helpers.
pub mod manifest {
    //! Manifest metadata for IVM smart contracts.
    //! Intended to be attached optionally to a transaction's `metadata`
    //! under a well-known key for admission-time checks.

    use iroha_crypto::{Hash, KeyPair, PublicKey, Signature};
    use iroha_schema::IntoSchema;
    use norito::codec::{Decode, Encode};
    #[cfg(feature = "json")]
    use norito::json::{self, FastJsonWrite, JsonDeserialize, JsonSerialize};

    /// Well-known metadata key used to attach a contract manifest.
    pub const MANIFEST_METADATA_KEY: &str = "contract_manifest";

    /// Minimal smart contract manifest used for admission-time validation.
    ///
    /// All fields are optional: when present they are verified; when absent they
    /// are ignored.
    #[derive(Debug, Clone, Encode, Decode, IntoSchema, PartialEq, Eq, PartialOrd, Ord)]
    #[norito(reuse_archived)]
    #[cfg_attr(
        feature = "json",
        derive(
            crate::DeriveFastJson,
            crate::DeriveJsonSerialize,
            crate::DeriveJsonDeserialize
        )
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub struct ContractManifest {
        /// Content-addressed hash of the compiled `.to` bytecode.
        /// If present, nodes compare it to the hash computed from the submitted bytecode.
        pub code_hash: Option<Hash>,
        /// ABI hash computed by the node for the `abi_version` policy.
        /// If present, must match the node's view of the syscall policy.
        pub abi_hash: Option<Hash>,
        /// Optional compiler fingerprint (e.g., rustc/LLVM versions).
        pub compiler_fingerprint: Option<String>,
        /// Feature bitmap used during compilation (e.g., SIMD/CUDA flags).
        pub features_bitmap: Option<u64>,
        /// Optional advisory access-set hints for scheduler.
        ///
        /// When present, the scheduler may use these read/write keys for conflict
        /// detection without requiring a dynamic VM prepass. Keys are canonical
        /// strings of the form `account:…`, `domain:…`, `asset_def:…`, `asset:…`,
        /// `nft:…`, or their `*.detail:…` variants, matching the internal
        /// pipeline access-key format.
        #[norito(default)]
        pub access_set_hints: Option<AccessSetHints>,
        /// Optional entrypoint descriptors (name, kind, permission) advertised by the compiler.
        #[norito(default)]
        pub entrypoints: Option<Vec<EntrypointDescriptor>>,
        /// Provenance metadata for the manifest, including signer and signature.
        #[norito(default)]
        pub provenance: Option<ManifestProvenance>,
    }

    /// Advisory read/write keys used by the scheduler when present in a manifest.
    #[derive(Debug, Clone, Encode, Decode, IntoSchema, PartialEq, Eq, PartialOrd, Ord)]
    pub struct AccessSetHints {
        /// Keys that the contract expects to read for a given entrypoint.
        pub read_keys: Vec<String>,
        /// Keys that the contract expects to write for a given entrypoint.
        pub write_keys: Vec<String>,
    }

    #[cfg(feature = "json")]
    impl FastJsonWrite for AccessSetHints {
        fn write_json(&self, out: &mut String) {
            out.push('{');
            json::write_json_string("read_keys", out);
            out.push(':');
            JsonSerialize::json_serialize(&self.read_keys, out);
            out.push(',');
            json::write_json_string("write_keys", out);
            out.push(':');
            JsonSerialize::json_serialize(&self.write_keys, out);
            out.push('}');
        }
    }

    #[cfg(feature = "json")]
    impl JsonDeserialize for AccessSetHints {
        fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
            parser.skip_ws();
            parser.consume_char(b'{')?;

            let mut read_keys: Option<Vec<String>> = None;
            let mut write_keys: Option<Vec<String>> = None;

            loop {
                parser.skip_ws();
                if parser.try_consume_char(b'}')? {
                    break;
                }

                let key = parser.parse_key()?;
                match key.as_str() {
                    "read_keys" => {
                        if read_keys.is_some() {
                            return Err(json::Error::duplicate_field("read_keys"));
                        }
                        read_keys = Some(Vec::<String>::json_deserialize(parser)?);
                    }
                    "write_keys" => {
                        if write_keys.is_some() {
                            return Err(json::Error::duplicate_field("write_keys"));
                        }
                        write_keys = Some(Vec::<String>::json_deserialize(parser)?);
                    }
                    other => {
                        return Err(json::Error::unknown_field(other));
                    }
                }

                if parser.consume_comma_if_present()? {
                    continue;
                }
                parser.skip_ws();
                parser.consume_char(b'}')?;
                break;
            }

            let read_keys = read_keys.ok_or_else(|| json::Error::missing_field("read_keys"))?;
            let write_keys = write_keys.ok_or_else(|| json::Error::missing_field("write_keys"))?;

            Ok(AccessSetHints {
                read_keys,
                write_keys,
            })
        }
    }

    /// Signature metadata binding a manifest to an approved signer.
    #[derive(Debug, Clone, Encode, Decode, IntoSchema, PartialEq, Eq, PartialOrd, Ord)]
    #[cfg_attr(
        feature = "json",
        derive(
            crate::DeriveFastJson,
            crate::DeriveJsonSerialize,
            crate::DeriveJsonDeserialize
        )
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub struct ManifestProvenance {
        /// Public key that signed the manifest payload.
        pub signer: PublicKey,
        /// Signature over the manifest payload (see [`ContractManifestSignaturePayload`]).
        pub signature: Signature,
    }

    /// Declarative metadata for a compiled entrypoint.
    #[derive(Debug, Clone, Encode, Decode, IntoSchema, PartialEq, Eq, PartialOrd, Ord)]
    #[cfg_attr(
        feature = "json",
        derive(
            crate::DeriveFastJson,
            crate::DeriveJsonSerialize,
            crate::DeriveJsonDeserialize
        )
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub struct EntrypointDescriptor {
        /// Symbol name as declared in the Kotodama source file.
        pub name: String,
        /// Logical kind: `kotoage`, `hajimari`, or `kaizen`.
        pub kind: EntryPointKind,
        /// Permission required by the dispatcher before invoking this entrypoint.
        #[norito(default)]
        pub permission: Option<String>,
        /// Advisory read keys for this entrypoint (flattened `state:...` strings).
        #[norito(default)]
        pub read_keys: Vec<String>,
        /// Advisory write keys for this entrypoint.
        #[norito(default)]
        pub write_keys: Vec<String>,
    }

    /// Entry point category advertised by Kotodama.
    #[derive(Debug, Clone, Copy, Encode, Decode, IntoSchema, PartialEq, Eq, PartialOrd, Ord)]
    #[cfg_attr(
        feature = "json",
        derive(
            crate::DeriveFastJson,
            crate::DeriveJsonSerialize,
            crate::DeriveJsonDeserialize
        )
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    #[norito(tag = "kind", content = "value")]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub enum EntryPointKind {
        /// Public dispatcher entrypoint (`kotoage fn`).
        Public,
        /// Deployment initializer (`hajimari`).
        Hajimari,
        /// Upgrade hook (`kaizen`).
        Kaizen,
    }

    /// Canonical payload signed to attest a manifest.
    #[derive(Debug, Clone, Encode, Decode, IntoSchema, PartialEq, Eq, PartialOrd, Ord)]
    #[cfg_attr(
        feature = "json",
        derive(
            crate::DeriveFastJson,
            crate::DeriveJsonSerialize,
            crate::DeriveJsonDeserialize
        )
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub struct ContractManifestSignaturePayload {
        /// Content-addressed hash of the compiled `.to` bytecode.
        pub code_hash: Option<Hash>,
        /// ABI hash computed by the node for the `abi_version` policy.
        pub abi_hash: Option<Hash>,
        /// Optional compiler fingerprint (e.g., rustc/LLVM versions).
        pub compiler_fingerprint: Option<String>,
        /// Feature bitmap used during compilation (e.g., SIMD/CUDA flags).
        pub features_bitmap: Option<u64>,
        /// Optional advisory access-set hints for scheduler.
        #[norito(default)]
        pub access_set_hints: Option<AccessSetHints>,
        /// Optional entrypoint descriptors (name, kind, permission) advertised by the compiler.
        #[norito(default)]
        pub entrypoints: Option<Vec<EntrypointDescriptor>>,
    }

    impl From<&ContractManifest> for ContractManifestSignaturePayload {
        fn from(manifest: &ContractManifest) -> Self {
            Self {
                code_hash: manifest.code_hash,
                abi_hash: manifest.abi_hash,
                compiler_fingerprint: manifest.compiler_fingerprint.clone(),
                features_bitmap: manifest.features_bitmap,
                access_set_hints: manifest.access_set_hints.clone(),
                entrypoints: manifest.entrypoints.clone(),
            }
        }
    }

    impl ContractManifest {
        /// Build the canonical payload that must be signed for provenance checks.
        #[must_use]
        pub fn signature_payload(&self) -> ContractManifestSignaturePayload {
            ContractManifestSignaturePayload::from(self)
        }

        /// Encode the canonical signing payload into Norito bytes.
        #[must_use]
        pub fn signature_payload_bytes(&self) -> Vec<u8> {
            norito::to_bytes(&self.signature_payload())
                .expect("manifest signature payload encoding must succeed")
        }

        /// Attach provenance by signing the canonical payload with the provided key pair.
        #[must_use]
        pub fn signed(mut self, key_pair: &KeyPair) -> Self {
            let payload = self.signature_payload_bytes();
            let signature = Signature::new(key_pair.private_key(), &payload);
            self.provenance = Some(ManifestProvenance {
                signer: key_pair.public_key().clone(),
                signature,
            });
            self
        }
    }

    #[cfg(all(test, feature = "json"))]
    mod tests {
        use super::*;

        #[test]
        fn access_set_hints_roundtrip() {
            let hints = AccessSetHints {
                read_keys: vec!["account:satoshi".to_owned()],
                write_keys: vec!["asset:btc#iroha".to_owned()],
            };

            let json = norito::json::to_json(&hints).expect("serialize access hints");
            assert_eq!(
                json,
                "{\"read_keys\":[\"account:satoshi\"],\"write_keys\":[\"asset:btc#iroha\"]}"
            );

            let decoded: AccessSetHints = norito::json::from_str(&json).expect("deserialize hints");
            assert_eq!(decoded.read_keys, hints.read_keys);
            assert_eq!(decoded.write_keys, hints.write_keys);
        }

        #[test]
        fn access_set_hints_missing_fields_fail() {
            let err = norito::json::from_str::<AccessSetHints>("{}")
                .expect_err("missing fields must fail");
            match err {
                norito::json::Error::MissingField { field } => {
                    assert_eq!(field, "read_keys", "unexpected field: {field}");
                }
                norito::json::Error::Message(msg) => {
                    assert!(msg.contains("read_keys"), "unexpected error: {msg}");
                }
                other => panic!("unexpected error: {other}"),
            }
        }
    }

    #[cfg(test)]
    mod manifest_signing_tests {
        use iroha_crypto::KeyPair;

        use super::*;

        #[test]
        fn signature_payload_excludes_provenance_and_verifies() {
            let kp = KeyPair::random();
            let mut manifest = ContractManifest {
                code_hash: Some(Hash::new(b"code-bytes")),
                abi_hash: Some(Hash::new(b"abi-bytes")),
                compiler_fingerprint: Some("rustc-1.78".to_owned()),
                features_bitmap: Some(0xAA),
                access_set_hints: None,
                entrypoints: None,
                provenance: None,
            };

            let payload = manifest.signature_payload_bytes();
            let signature = Signature::new(kp.private_key(), &payload);
            manifest.provenance = Some(ManifestProvenance {
                signer: kp.public_key().clone(),
                signature: signature.clone(),
            });

            // Provenance should not affect the payload bytes.
            assert_eq!(payload, manifest.signature_payload_bytes());
            signature
                .verify(kp.public_key(), &payload)
                .expect("signature must verify");
        }
    }
}
