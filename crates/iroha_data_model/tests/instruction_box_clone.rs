//! Tests for `InstructionBox` cloning.

use iroha_data_model::{
    instruction_registry,
    isi::{InstructionBox, SetParameter, set_instruction_registry},
    parameter::{BlockParameter, Parameter},
};
use nonzero_ext::nonzero;

#[test]
fn clone_roundtrip_set_parameter() {
    set_instruction_registry(instruction_registry![SetParameter]);
    let isi = SetParameter::new(Parameter::Block(BlockParameter::MaxTransactions(nonzero!(
        1_u64
    ))));
    let boxed = InstructionBox::from(isi);
    let cloned = boxed.clone();
    assert_eq!(boxed, cloned);
}
