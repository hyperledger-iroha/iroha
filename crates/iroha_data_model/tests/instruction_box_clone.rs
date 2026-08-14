//! Tests for `InstructionBox` cloning.
use iroha_data_model::{
    isi::{InstructionBox, SetParameter},
    parameter::{BlockParameter, Parameter},
};
use nonzero_ext::nonzero;
#[test]
fn clone_roundtrip_set_parameter() {
    let isi = SetParameter::new(Parameter::Block(BlockParameter::MaxTransactions(nonzero!(
        1_u64
    ))));
    let boxed = InstructionBox::from(isi);
    let cloned = boxed.clone();
    assert_eq!(boxed, cloned);
}
