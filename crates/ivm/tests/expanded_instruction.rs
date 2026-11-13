use ivm::{encoding, instruction};
#[test]
fn wide_field_extractors() {
    let word = encoding::wide::encode_rr(instruction::wide::arithmetic::ADD, 31, 19, 7);
    assert_eq!(
        instruction::wide::opcode(word),
        instruction::wide::arithmetic::ADD
    );
    assert_eq!(instruction::wide::rd(word), 31_usize);
    assert_eq!(instruction::wide::rs1(word), 19_usize);
    assert_eq!(instruction::wide::rs2(word), 7_usize);
    assert_eq!(word & 0x1ff, 0, "Register3 reserves bits 8..0");
}
