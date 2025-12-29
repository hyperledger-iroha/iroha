use ivm::Registers;

#[test]
fn register_merkle_updates() {
    let mut regs = Registers::new();
    let initial = regs.merkle_root();
    regs.set(1, 42);
    let r1 = regs.merkle_root();
    assert_ne!(initial, r1);
    regs.set(2, 42);
    let r2 = regs.merkle_root();
    assert_ne!(r1, r2);
    regs.set(1, 0);
    regs.set(2, 0);
    assert_eq!(regs.merkle_root(), initial);
}
