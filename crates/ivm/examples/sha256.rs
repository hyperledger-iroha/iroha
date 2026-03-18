//! Example: compute a single SHA-256 block using an IVM custom op.
use ivm::IVM;

fn main() {
    let mut vm = IVM::new(u64::MAX);
    // Prepare a single-block message "abc" with SHA-256 padding
    let mut block = [0u8; 64];
    // "abc" in ASCII
    block[0] = 0x61;
    block[1] = 0x62;
    block[2] = 0x63;
    // Append 0x80
    block[3] = 0x80;
    // Remaining bytes [4..56) stay 0x00
    // Length in bits (24) as 64-bit big-endian at end
    block[63] = 24; // low 8 bits of length
    // (block[56..63] except last are 0)
    // Set initial SHA-256 state (IV values)
    let iv: [u32; 8] = [
        0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A, 0x510E527F, 0x9B05688C, 0x1F83D9AB,
        0x5BE0CD19,
    ];
    vm.set_vector_register(0, [iv[0], iv[1], iv[2], iv[3]]);
    vm.set_vector_register(1, [iv[4], iv[5], iv[6], iv[7]]);
    // Copy message block into VM memory (heap region)
    let addr = ivm::Memory::HEAP_START;
    for (i, byte) in block.iter().enumerate() {
        vm.store_u8(addr + i as u64, *byte)
            .expect("Memory store failed");
    }
    // Program: SHA256BLOCK (using v0 and memory at addr); HALT
    // We'll encode the custom instruction as: opcode 0x5B, rs1 = x1 (pointer), rd = v0 index
    // Set x1 = address of the block
    vm.set_register(1, addr);
    // Construct 32-bit instruction for SHA256BLOCK v0, (x1):
    // For simplicity, we choose rd=0 for v0, rs1=1 for pointer
    let instr: u32 = (1 << 15) | 0x5B; // rs1=x1, rd=v0(0), opcode=0x5B
    let body: [u8; 8] = [
        (instr & 0xFF) as u8,
        ((instr >> 8) & 0xFF) as u8,
        ((instr >> 16) & 0xFF) as u8,
        ((instr >> 24) & 0xFF) as u8,
        0x00,
        0x00,
        0x00,
        0x00, // halt
    ];
    let mut program = ivm::ProgramMetadata::default().encode();
    program.extend_from_slice(&body);
    vm.load_program(&program).unwrap();
    vm.run().expect("VM execution failed");
    // Retrieve the resulting hash from v0:v1
    let mut hash_words = [0u32; 8];
    hash_words[..4].copy_from_slice(&vm.vector_register(0));
    hash_words[4..].copy_from_slice(&vm.vector_register(1));
    // Convert to 32 bytes (big-endian per word)
    let mut hash_bytes = [0u8; 32];
    for (i, &word) in hash_words.iter().enumerate() {
        hash_bytes[4 * i..4 * i + 4].copy_from_slice(&word.to_be_bytes());
    }
    // Expected SHA-256 digest of "abc"
    let expected_hex = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    let got_hex: String = hash_bytes.iter().map(|b| format!("{b:02x}")).collect();
    if got_hex == expected_hex {
        println!("SHA256 computed correctly: {got_hex}");
    } else {
        println!("SHA256 computation incorrect!");
        println!("Expected: {expected_hex}");
        println!("Got:      {got_hex}");
    }
}
