use ivm::{encoding, instruction};

#[test]
fn wide_field_extractors() {
    let word = encoding::wide::encode_rr(instruction::wide::arithmetic::ADD, 0xAB, 0xCD, 0xEF);
    assert_eq!(
        instruction::wide::opcode(word),
        instruction::wide::arithmetic::ADD
    );
    assert_eq!(instruction::wide::rd(word), 0xAB_usize);
    assert_eq!(instruction::wide::rs1(word), 0xCD_usize);
    assert_eq!(instruction::wide::rs2(word), 0xEF_usize);
}
