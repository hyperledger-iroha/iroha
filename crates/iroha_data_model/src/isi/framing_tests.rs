//! Instruction tuple framing, counting propagation, and registry ownership contracts.

use std::{cell::Cell, sync::Arc};

use iroha_primitives::const_vec::ConstVec;
use norito::core::{
    Archived, DecodeFlagsGuard, Encoder, Error, Header, NoritoDeserialize, NoritoSerialize,
    encoded_payload_len, from_bytes, serialize_to_buffer, supported_header_flags, to_bytes,
    validate_header_flags,
};

use super::{INSTRUCTION_REGISTRY_OVERRIDE, InstructionBox, InstructionRegistry};

const COUNTED_WIRE_ID: &str = "test.instruction.v1::CountedFrame";

thread_local! {
    static SERIALIZE_VISITS: Cell<usize> = const { Cell::new(0) };
}

#[derive(Clone, Debug, PartialEq, PartialOrd)]
struct CountedInstruction(u8);

impl crate::seal::Instruction for CountedInstruction {}

impl NoritoSerialize for CountedInstruction {
    fn schema_hash() -> [u8; 16] {
        <u8 as NoritoSerialize>::schema_hash()
    }

    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        SERIALIZE_VISITS.with(|visits| visits.set(visits.get() + 1));
        self.0.serialize(writer)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        panic!("instruction frame measurement must not trust a length hint")
    }
}

impl<'a> NoritoDeserialize<'a> for CountedInstruction {
    fn schema_hash() -> [u8; 16] {
        <u8 as NoritoSerialize>::schema_hash()
    }

    fn deserialize(archived: &'a Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("decode counted instruction")
    }

    fn try_deserialize(archived: &'a Archived<Self>) -> Result<Self, Error> {
        u8::try_deserialize(archived.cast()).map(Self)
    }
}

struct RegistryGuard(Option<Arc<InstructionRegistry>>);

impl RegistryGuard {
    fn enter() -> Self {
        let registry =
            InstructionRegistry::new().register_with_id::<CountedInstruction>(COUNTED_WIRE_ID);
        Self(INSTRUCTION_REGISTRY_OVERRIDE.with(|slot| slot.replace(Some(Arc::new(registry)))))
    }
}

impl Drop for RegistryGuard {
    fn drop(&mut self) {
        INSTRUCTION_REGISTRY_OVERRIDE.with(|slot| {
            *slot.borrow_mut() = self.0.take();
        });
    }
}

fn layouts() -> impl Iterator<Item = u8> {
    (0..=supported_header_flags()).filter(|flags| validate_header_flags(*flags).is_ok())
}

#[test]
fn instruction_box_and_const_vec_measure_each_instruction_once() {
    let _registry = RegistryGuard::enter();
    for flags in layouts() {
        let _flags = DecodeFlagsGuard::enter(flags);
        let boxed = InstructionBox(Box::new(CountedInstruction(0xa5)));
        let sequence = ConstVec::from(vec![boxed.clone()]);
        let cases: [(&dyn NoritoSerialize, usize); 2] = [(&boxed, 2), (&sequence, 3)];
        for (value, output_visits) in cases {
            SERIALIZE_VISITS.with(|visits| visits.set(0));
            let length = encoded_payload_len(value).expect("measure instruction payload");
            assert_eq!(
                SERIALIZE_VISITS.with(Cell::get),
                1,
                "counting replayed the framed instruction, flags {flags:#x}"
            );
            SERIALIZE_VISITS.with(|visits| visits.set(0));
            let mut bytes = Vec::new();
            serialize_to_buffer(value, &mut bytes).expect("emit checked instruction payload");
            assert_eq!(bytes.len(), length);
            assert_eq!(
                SERIALIZE_VISITS.with(Cell::get),
                output_visits,
                "actual output added a redundant frame-length pass, flags {flags:#x}"
            );
        }
    }
}

#[test]
fn instruction_box_preserves_scalar_frame_and_tuple_bytes_in_every_layout() {
    let _registry = RegistryGuard::enter();
    for flags in layouts() {
        let _flags = DecodeFlagsGuard::enter(flags);
        let boxed = InstructionBox(Box::new(CountedInstruction(0xa5)));
        // The fixture instruction has the scalar u8 wire/schema contract. This
        // independent buffer encoder pins its embedded header, checksum and body.
        let scalar_frame = to_bytes(&0xa5_u8).expect("canonical scalar frame");
        let pair = (COUNTED_WIRE_ID.to_owned(), scalar_frame);
        let mut expected = Vec::new();
        serialize_to_buffer(&pair, &mut expected).expect("canonical tuple fields");
        let mut actual = Vec::new();
        serialize_to_buffer(&boxed, &mut actual).expect("instruction tuple");
        assert_eq!(
            actual, expected,
            "instruction tuple changed, flags {flags:#x}"
        );
        assert_eq!(encoded_payload_len(&boxed).unwrap(), expected.len());

        let frame = to_bytes(&boxed).expect("frame instruction box");
        let header = Header::read(&mut &frame[..]).expect("instruction box header");
        let _frame_flags = DecodeFlagsGuard::enter(header.flags);
        let archived =
            from_bytes::<InstructionBox>(&frame).expect("validate instruction box frame");
        let decoded =
            InstructionBox::try_deserialize(archived).expect("decode registered instruction");
        assert_eq!(
            decoded.as_any().downcast_ref::<CountedInstruction>(),
            Some(&CountedInstruction(0xa5))
        );
    }
}
