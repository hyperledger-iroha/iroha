//! Types representing executable parts of a transaction.

use std::{iter::IntoIterator, vec::Vec};

use ::base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_data_model_derive::model;
use iroha_primitives::json::Json;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

pub use self::model::*;
#[cfg(test)]
use crate::isi::Instruction;
use crate::isi::InstructionBox;
use crate::smart_contract::ContractAddress;

#[model]
mod model {
    use iroha_crypto::Hash;
    use iroha_primitives::const_vec::ConstVec;

    use super::*;

    /// Either ISI or IVM smart contract bytecode
    #[derive(
        derive_more::Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub enum Executable {
        /// Ordered set of instructions.
        #[cfg_attr(not(feature = "fast_dsl"), debug("{_0:?}"))]
        #[cfg_attr(feature = "fast_dsl", debug("Instructions(..)"))]
        Instructions(ConstVec<InstructionBox>),
        /// Invoke a deployed contract instance by reference.
        ContractCall(ContractInvocation),
        /// IVM smart contract bytecode (.to)
        Ivm(IvmBytecode),
        /// IVM smart contract bytecode accompanied by a precomputed instruction overlay.
        ///
        /// This executable is intended for proof-carrying flows where the transaction
        /// supplies a deterministic overlay (ISIs) together with a ZK proof (via
        /// [`SignedTransaction`](crate::transaction::SignedTransaction) attachments) that
        /// binds the overlay to the executed bytecode.
        ///
        /// Nodes verify the proof and may deterministically replay the IVM execution as an
        /// additional safety check depending on pipeline policy.
        IvmProved(IvmProved),
    }

    /// Wrapper for IVM bytecode used by [`Executable::Ivm`].
    ///
    /// Uses **base64** (de-)serialization format.
    #[derive(
        derive_more::Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema,
    )]
    #[debug("IVM bytecode(len = {})", self.0.len())]
    #[cfg_attr(feature = "json", norito(transparent))]
    #[repr(transparent)]
    // SAFETY: `IvmBytecode` has no trap representation in `Vec<u8>`
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(unsafe {robust}))]
    pub struct IvmBytecode(
        /// Raw Kotodama bytecode blob.
        pub(super) Vec<u8>,
    );

    /// Wrapper for proved IVM executions.
    #[derive(
        derive_more::Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct IvmProved {
        /// Raw Kotodama bytecode blob.
        pub bytecode: IvmBytecode,
        /// Precomputed ordered instruction overlay to apply when the proof verifies.
        pub overlay: ConstVec<InstructionBox>,
        /// Commitment to deterministic execution-side events materialized for this proved run.
        pub events_commitment: Hash,
        /// Commitment to gas policy compliance (without revealing exact gas usage).
        pub gas_policy_commitment: Hash,
    }

    /// By-reference invocation of a deployed contract instance.
    #[derive(
        derive_more::Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema,
    )]
    #[cfg_attr(
        feature = "json",
        derive(
            crate::DeriveFastJson,
            crate::DeriveJsonSerialize,
            crate::DeriveJsonDeserialize
        )
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    pub struct ContractInvocation {
        /// Canonical deployed contract address.
        pub contract_address: ContractAddress,
        /// Public or view entrypoint selector.
        pub entrypoint: String,
        /// Optional Norito JSON payload forwarded to the contract.
        #[norito(default)]
        pub payload: Option<Json>,
    }
}

// Collect any iterator of instructions into an executable, avoiding
// double-boxing when items are already `InstructionBox`.
impl<A> FromIterator<A> for Executable
where
    A: Into<InstructionBox>,
{
    fn from_iter<T: IntoIterator<Item = A>>(iter: T) -> Self {
        let items: Vec<InstructionBox> = iter.into_iter().map(Into::into).collect();
        Self::Instructions(items.into())
    }
}

impl<T, A> From<T> for Executable
where
    T: IntoIterator<Item = A>,
    A: Into<InstructionBox>,
{
    fn from(collection: T) -> Self {
        Executable::from_iter(collection)
    }
}

impl From<IvmBytecode> for Executable {
    fn from(source: IvmBytecode) -> Self {
        Self::Ivm(source)
    }
}

impl From<ContractInvocation> for Executable {
    fn from(source: ContractInvocation) -> Self {
        Self::ContractCall(source)
    }
}

impl AsRef<[u8]> for IvmBytecode {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl IvmBytecode {
    /// Create [`Self`] from raw IVM bytecode
    #[inline]
    pub const fn from_compiled(blob: Vec<u8>) -> Self {
        Self(blob)
    }

    /// Size of the smart contract in bytes
    pub fn size_bytes(&self) -> usize {
        self.0.len()
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for IvmBytecode {
    fn write_json(&self, out: &mut String) {
        let encoded = STANDARD.encode(&self.0);
        norito::json::JsonSerialize::json_serialize(&encoded, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for IvmBytecode {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let encoded = parser.parse_string()?;
        let bytes = STANDARD
            .decode(encoded.as_str())
            .map_err(|err| norito::json::Error::Message(err.to_string()))?;
        Ok(Self(bytes))
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for IvmProved {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        norito::json::write_json_string("bytecode", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.bytecode, out);
        out.push(',');
        norito::json::write_json_string("overlay", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.overlay, out);
        out.push(',');
        norito::json::write_json_string("events_commitment", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.events_commitment, out);
        out.push(',');
        norito::json::write_json_string("gas_policy_commitment", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.gas_policy_commitment, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for IvmProved {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        parser.skip_ws();
        parser.consume_char(b'{')?;
        let mut bytecode: Option<IvmBytecode> = None;
        let mut overlay: Option<iroha_primitives::const_vec::ConstVec<InstructionBox>> = None;
        let mut events_commitment: Option<iroha_crypto::Hash> = None;
        let mut gas_policy_commitment: Option<iroha_crypto::Hash> = None;
        loop {
            parser.skip_ws();
            if parser.try_consume_char(b'}')? {
                break;
            }
            let field = parser.parse_key()?;
            match field.as_str() {
                "bytecode" => {
                    bytecode = Some(IvmBytecode::json_deserialize(parser)?);
                }
                "overlay" => {
                    overlay = Some(
                        iroha_primitives::const_vec::ConstVec::<InstructionBox>::json_deserialize(
                            parser,
                        )?,
                    );
                }
                "events_commitment" => {
                    events_commitment = Some(iroha_crypto::Hash::json_deserialize(parser)?);
                }
                "gas_policy_commitment" => {
                    gas_policy_commitment = Some(iroha_crypto::Hash::json_deserialize(parser)?);
                }
                other => return Err(norito::json::Error::unknown_field(other.to_owned())),
            }
            if !parser.consume_comma_if_present()? {
                parser.skip_ws();
                parser.consume_char(b'}')?;
                break;
            }
        }
        let bytecode = bytecode
            .ok_or_else(|| norito::json::Error::Message("missing field `bytecode`".to_owned()))?;
        let overlay = overlay
            .ok_or_else(|| norito::json::Error::Message("missing field `overlay`".to_owned()))?;
        let events_commitment = events_commitment.ok_or_else(|| {
            norito::json::Error::Message("missing field `events_commitment`".to_owned())
        })?;
        let gas_policy_commitment = gas_policy_commitment.ok_or_else(|| {
            norito::json::Error::Message("missing field `gas_policy_commitment`".to_owned())
        })?;
        Ok(Self {
            bytecode,
            overlay,
            events_commitment,
            gas_policy_commitment,
        })
    }
}

impl Executable {
    /// Number of instructions if this executable is an ISI batch; `0` for IVM bytecode.
    pub fn instruction_count(&self) -> u64 {
        match self {
            Executable::Instructions(instructions) => instructions.len() as u64,
            Executable::ContractCall(_) => 0,
            Executable::Ivm(_) => 0,
            Executable::IvmProved(proved) => proved.overlay.len() as u64,
        }
    }

    /// Returns bytecode size if this is `Executable::Ivm`, otherwise `0`.
    pub fn ivm_size_bytes(&self) -> usize {
        match self {
            Executable::Ivm(b) => b.size_bytes(),
            Executable::ContractCall(_) => 0,
            Executable::Instructions(_) => 0,
            Executable::IvmProved(proved) => proved.bytecode.size_bytes(),
        }
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for Executable {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        parser.skip_ws();
        parser.consume_char(b'{')?;
        parser.skip_ws();
        let key = parser.parse_key()?;
        let exec = match key.as_str() {
            "Instructions" => {
                let instrs =
                    iroha_primitives::const_vec::ConstVec::<InstructionBox>::json_deserialize(
                        parser,
                    )?;
                Executable::Instructions(instrs)
            }
            "ContractCall" => {
                Executable::ContractCall(ContractInvocation::json_deserialize(parser)?)
            }
            "Ivm" => Executable::Ivm(IvmBytecode::json_deserialize(parser)?),
            "IvmProved" => {
                parser.skip_ws();
                parser.consume_char(b'{')?;
                let mut bytecode: Option<IvmBytecode> = None;
                let mut overlay: Option<iroha_primitives::const_vec::ConstVec<InstructionBox>> =
                    None;
                let mut events_commitment: Option<iroha_crypto::Hash> = None;
                let mut gas_policy_commitment: Option<iroha_crypto::Hash> = None;
                loop {
                    parser.skip_ws();
                    if parser.try_consume_char(b'}')? {
                        break;
                    }
                    let field = parser.parse_key()?;
                    match field.as_str() {
                        "bytecode" => {
                            bytecode = Some(IvmBytecode::json_deserialize(parser)?);
                        }
                        "overlay" => {
                            overlay = Some(
                                iroha_primitives::const_vec::ConstVec::<InstructionBox>::json_deserialize(parser)?,
                            );
                        }
                        "events_commitment" => {
                            events_commitment = Some(iroha_crypto::Hash::json_deserialize(parser)?);
                        }
                        "gas_policy_commitment" => {
                            gas_policy_commitment =
                                Some(iroha_crypto::Hash::json_deserialize(parser)?);
                        }
                        other => {
                            return Err(norito::json::Error::unknown_field(other.to_owned()));
                        }
                    }
                    if !parser.consume_comma_if_present()? {
                        parser.skip_ws();
                        parser.consume_char(b'}')?;
                        break;
                    }
                }
                let bytecode = bytecode.ok_or_else(|| {
                    norito::json::Error::Message("missing field `bytecode`".to_owned())
                })?;
                let overlay = overlay.ok_or_else(|| {
                    norito::json::Error::Message("missing field `overlay`".to_owned())
                })?;
                let events_commitment = events_commitment.ok_or_else(|| {
                    norito::json::Error::Message("missing field `events_commitment`".to_owned())
                })?;
                let gas_policy_commitment = gas_policy_commitment.ok_or_else(|| {
                    norito::json::Error::Message("missing field `gas_policy_commitment`".to_owned())
                })?;
                Executable::IvmProved(IvmProved {
                    bytecode,
                    overlay,
                    events_commitment,
                    gas_policy_commitment,
                })
            }
            other => return Err(norito::json::Error::unknown_field(other.to_owned())),
        };
        parser.skip_ws();
        parser.consume_char(b'}')?;
        Ok(exec)
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for Executable {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        match self {
            Executable::Instructions(instrs) => {
                norito::json::write_json_string("Instructions", out);
                out.push(':');
                norito::json::JsonSerialize::json_serialize(instrs, out);
            }
            Executable::ContractCall(invocation) => {
                norito::json::write_json_string("ContractCall", out);
                out.push(':');
                norito::json::JsonSerialize::json_serialize(invocation, out);
            }
            Executable::Ivm(bytecode) => {
                norito::json::write_json_string("Ivm", out);
                out.push(':');
                norito::json::JsonSerialize::json_serialize(bytecode, out);
            }
            Executable::IvmProved(proved) => {
                norito::json::write_json_string("IvmProved", out);
                out.push(':');
                out.push('{');
                norito::json::write_json_string("bytecode", out);
                out.push(':');
                norito::json::JsonSerialize::json_serialize(&proved.bytecode, out);
                out.push(',');
                norito::json::write_json_string("overlay", out);
                out.push(':');
                norito::json::JsonSerialize::json_serialize(&proved.overlay, out);
                out.push(',');
                norito::json::write_json_string("events_commitment", out);
                out.push(':');
                norito::json::JsonSerialize::json_serialize(&proved.events_commitment, out);
                out.push(',');
                norito::json::write_json_string("gas_policy_commitment", out);
                out.push(':');
                norito::json::JsonSerialize::json_serialize(&proved.gas_policy_commitment, out);
                out.push('}');
            }
        }
        out.push('}');
    }
}

#[cfg(test)]
mod tests {
    use std::any::Any;

    use super::*;

    #[derive(Debug, Clone)]
    struct DummyInstruction(pub u32);

    impl crate::seal::Instruction for DummyInstruction {}

    impl Instruction for DummyInstruction {
        fn dyn_encode(&self) -> Vec<u8> {
            Vec::new()
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    // Provide a local conversion so tests can collect DummyInstruction
    // directly into an Executable without extra boilerplate.
    impl From<DummyInstruction> for InstructionBox {
        fn from(i: DummyInstruction) -> Self {
            Instruction::into_instruction_box(Box::new(i))
        }
    }

    #[test]
    fn ivm_bytecode_debug_repr_should_contain_just_len() {
        // IVM bytecode debug output should only show its length
        let ivm_bytecode = IvmBytecode::from_compiled(vec![0, 1, 2, 3, 4]);
        assert_eq!(format!("{ivm_bytecode:?}"), "IVM bytecode(len = 5)");
    }

    #[test]

    fn executable_from_iter_should_preserve_order() {
        let executable = Executable::from_iter(vec![
            DummyInstruction(1),
            DummyInstruction(2),
            DummyInstruction(3),
        ]);

        let Executable::Instructions(instructions) = executable else {
            panic!("expected instructions variant");
        };

        let ids: Vec<u32> = instructions
            .into_iter()
            .map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<DummyInstruction>()
                    .unwrap()
                    .0
            })
            .collect();

        assert_eq!(ids, vec![1, 2, 3]);
    }
    #[cfg(feature = "json")]
    #[test]
    fn ivm_bytecode_should_serialize_and_deserialize() {
        let bytecode = IvmBytecode::from_compiled(vec![1, 2, 3, 4, 5]);
        let json = norito::json::to_json(&bytecode).expect("serialize");
        let deserialized: IvmBytecode = norito::json::from_str(&json).expect("deserialize");
        assert_eq!(bytecode, deserialized);
    }

    #[cfg(feature = "json")]
    #[test]
    fn executable_json_roundtrip_for_instructions_and_ivm() {
        let instruction: InstructionBox =
            crate::isi::Log::new(crate::Level::INFO, "json executable".into()).into();
        let executable = Executable::from_iter([instruction]);
        let json = norito::json::to_json(&executable).expect("serialize instructions");
        let deserialized: Executable = norito::json::from_str(&json).expect("deserialize");
        assert_eq!(executable, deserialized);

        let ivm_executable = Executable::Ivm(IvmBytecode::from_compiled(vec![9, 8, 7]));
        let json = norito::json::to_json(&ivm_executable).expect("serialize ivm");
        let deserialized: Executable = norito::json::from_str(&json).expect("deserialize ivm");
        assert_eq!(ivm_executable, deserialized);

        let contract_call_executable = Executable::ContractCall(ContractInvocation {
            contract_address: "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8"
                .parse()
                .expect("contract address"),
            entrypoint: "contribute".to_owned(),
            payload: Some(Json::new(norito::json!({
                "sale": "genesis_sale",
                "payment_amount": 1
            }))),
        });
        let json =
            norito::json::to_json(&contract_call_executable).expect("serialize contract call");
        let deserialized: Executable =
            norito::json::from_str(&json).expect("deserialize contract call");
        assert_eq!(contract_call_executable, deserialized);

        let proved_executable = Executable::IvmProved(IvmProved {
            bytecode: IvmBytecode::from_compiled(vec![7, 7, 7]),
            overlay: Vec::<InstructionBox>::new().into(),
            events_commitment: iroha_crypto::Hash::new(b"events"),
            gas_policy_commitment: iroha_crypto::Hash::new(b"gas-policy"),
        });
        let json = norito::json::to_json(&proved_executable).expect("serialize proved");
        let deserialized: Executable = norito::json::from_str(&json).expect("deserialize proved");
        assert_eq!(proved_executable, deserialized);
    }
}
