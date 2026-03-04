use ivm::{IVM, encoding, instruction};
mod common;
use common::assemble;

const HALT: [u8; 4] = encoding::wide::encode_halt().to_le_bytes();

#[test]
fn test_vote_commitment_aggregation() {
    // voter1: weight 5 yes, randomness 4
    // voter2: weight 3 yes, randomness 9
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(1, 5); // value1
    vm.set_register(2, 4); // rand1
    vm.set_register(3, 3); // value2
    vm.set_register(4, 9); // rand2

    let mut prog = Vec::new();
    // VALCOM r5, r1, r2
    let valcom1 = encoding::wide::encode_rr(instruction::wide::crypto::VALCOM, 5, 1, 2);
    prog.extend_from_slice(&valcom1.to_le_bytes());
    // VALCOM r6, r3, r4
    let valcom2 = encoding::wide::encode_rr(instruction::wide::crypto::VALCOM, 6, 3, 4);
    prog.extend_from_slice(&valcom2.to_le_bytes());
    // ECADD r7, r5, r6
    let ecadd = encoding::wide::encode_rr(instruction::wide::crypto::ECADD, 7, 5, 6);
    prog.extend_from_slice(&ecadd.to_le_bytes());
    prog.extend_from_slice(&HALT);

    let prog = assemble(&prog);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();

    let c1 = ivm::pedersen_commit_truncated(5, 4);
    let c2 = ivm::pedersen_commit_truncated(3, 9);
    let expected = ivm::ec::ec_add_truncated(c1, c2);
    assert_eq!(vm.register(7), expected);
}
