//! Types representing executable parts of a transaction.

use std::{iter::IntoIterator, vec::Vec};

use ::base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_data_model_derive::model;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

pub use self::model::*;
#[cfg(test)]
use crate::isi::Instruction;
use crate::isi::InstructionBox;

#[model]
mod model {
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
        /// IVM smart contract bytecode (.to)
        Ivm(IvmBytecode),
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

impl Executable {
    /// Number of instructions if this executable is an ISI batch; `0` for IVM bytecode.
    pub fn instruction_count(&self) -> u64 {
        match self {
            Executable::Instructions(instructions) => instructions.len() as u64,
            Executable::Ivm(_) => 0,
        }
    }

    /// Returns bytecode size if this is `Executable::Ivm`, otherwise `0`.
    pub fn ivm_size_bytes(&self) -> usize {
        match self {
            Executable::Ivm(b) => b.size_bytes(),
            Executable::Instructions(_) => 0,
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
            "Ivm" => Executable::Ivm(IvmBytecode::json_deserialize(parser)?),
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
            Executable::Ivm(bytecode) => {
                norito::json::write_json_string("Ivm", out);
                out.push(':');
                norito::json::JsonSerialize::json_serialize(bytecode, out);
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
    }
}
