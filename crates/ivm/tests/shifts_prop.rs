use ivm::{IVM, encoding, instruction};

fn push32(code: &mut Vec<u8>, word: u32) {
    code.extend_from_slice(&word.to_le_bytes());
}
fn halt32(code: &mut Vec<u8>) {
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
}

fn wide_rr(op: u8, rd: u8, rs1: u8, rs2: u8) -> u32 {
    encoding::wide::encode_rr(op, rd, rs1, rs2)
}

// Tiny deterministic PRNG (splitmix64)
#[derive(Clone, Copy)]
struct SplitMix64(u64);
impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next(&mut self) -> u64 {
        let mut z = {
            self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
            self.0
        };
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
}

#[test]
fn shifts_rtype_property_random() {
    // Program computes SRL, SRA, SLL: r10=r1>>r3 (logical), r11=r1>>r3 (arith), r12=r1<<r3
    let mut code = Vec::new();
    push32(
        &mut code,
        wide_rr(instruction::wide::arithmetic::SRL, 10, 1, 3),
    );
    push32(
        &mut code,
        wide_rr(instruction::wide::arithmetic::SRA, 11, 1, 3),
    );
    push32(
        &mut code,
        wide_rr(instruction::wide::arithmetic::SLL, 12, 1, 3),
    );
    halt32(&mut code);

    // Deterministic seed for reproducibility
    let mut rng = SplitMix64::new(0xDEAD_BEEF_CAFE_BABE);
    // Cover a decent set of random values and shifts
    for _ in 0..512 {
        let val = rng.next();
        let shamt_rand = rng.next() & 0x7F; // up to 127 to exercise masking
        let shamt_masked = (shamt_rand & 0x3F) as u32; // VM masks to low 6 bits

        let mut vm = IVM::new(50_000);
        vm.memory.load_code(&code);
        vm.registers.set(1, val);
        vm.registers.set(3, shamt_rand);
        vm.run().unwrap();

        let got_srl = vm.registers.get(10);
        let got_sra = vm.registers.get(11);
        let got_sll = vm.registers.get(12);

        let exp_srl = val >> shamt_masked;
        let exp_sll = val << shamt_masked;
        let exp_sra = ((val as i64) >> shamt_masked) as u64;

        assert_eq!(
            got_srl, exp_srl,
            "SRL mismatch: val={val:#x} shamt={shamt_rand} (masked={shamt_masked})"
        );
        assert_eq!(
            got_sra, exp_sra,
            "SRA mismatch: val={val:#x} shamt={shamt_rand} (masked={shamt_masked})"
        );
        assert_eq!(
            got_sll, exp_sll,
            "SLL mismatch: val={val:#x} shamt={shamt_rand} (masked={shamt_masked})"
        );
    }
}
