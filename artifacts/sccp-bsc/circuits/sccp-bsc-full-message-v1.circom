pragma circom 2.1.6;

// SCCP BSC full-message circuit profile:
// sccp-bsc-full-message-v1
//
// This source mirrors SccpGroth16Bn254MessageVerifier._publicSignals:
// publicSignals[i] = uint256(keccak256(abi.encode(label[i], value[i]))) mod Fr.
// valueBits are bytes32 words in ABI byte order, with bits little-endian inside
// each byte. Byte 0 is the most significant ABI byte.
//
// Required external gadgets:
//   circomlib/circuits/gates.circom
//   circomlib/circuits/sha256/xor3.circom
//   circomlib/circuits/sha256/shift.circom
//   circomlib/circuits/bitify.circom
//   @electron-labs/keccak-circom/circuits/keccak.circom
//
// Solidity verifier signals:
//   message_id                  = keccak256(0x091b1715f31adbc0239378caf77a4370e8348599048ec45efb203368dbcc5073 || value) mod Fr
//   payload_hash                = keccak256(0xd40cf4310af21ab1b3f12db20df99ab8fe63dbe55fc473e5456691c39c1859ac || value) mod Fr
//   target_domain               = keccak256(0x5f7c135fa34a3f53c3733c64f172ef8a639790cfe240c9b454311f8cbfe74f96 || value) mod Fr
//   commitment_root             = keccak256(0xc3aa105618977410007f32f4eefe0b3eab174af6dac0d95829b92e18912bfbe3 || value) mod Fr
//   finality_height             = keccak256(0x0d3499b9350c0ac6add6e0076775de67baee79c5b691f3a4f9317dcb974db599 || value) mod Fr
//   finality_block_hash         = keccak256(0x1c5d4645e72d75c0152153a5fe8679a3c0a7ba6cfe3b91986e647c4b26c144bc || value) mod Fr
//   source_domain               = keccak256(0xd07ef0087259b42adc11497be275f42091c6ef51becccd113be860e1b48a5109 || value) mod Fr
//   statement_hash              = keccak256(0xa4895607d62c8e116357ba7d102e08b5636840e0816a608f3a1fc9d0a1077569 || value) mod Fr
//   destination_binding_hash    = keccak256(0x094cf24d193ac65c8a450188d16282fba8ee8c5a7539b751857d231f4380c2dd || value) mod Fr

include "circomlib/circuits/gates.circom";
include "circomlib/circuits/sha256/xor3.circom";
include "circomlib/circuits/sha256/shift.circom";
include "circomlib/circuits/bitify.circom";
include "@electron-labs/keccak-circom/circuits/keccak.circom";

template SccpBscLabeledKeccakSignal(label0, label1, label2, label3, label4, label5, label6, label7, label8, label9, label10, label11, label12, label13, label14, label15, label16, label17, label18, label19, label20, label21, label22, label23, label24, label25, label26, label27, label28, label29, label30, label31) {
  signal input valueBits[256];
  signal input publicSignal;

  var labelBytes[32];
  labelBytes[0] = label0;
  labelBytes[1] = label1;
  labelBytes[2] = label2;
  labelBytes[3] = label3;
  labelBytes[4] = label4;
  labelBytes[5] = label5;
  labelBytes[6] = label6;
  labelBytes[7] = label7;
  labelBytes[8] = label8;
  labelBytes[9] = label9;
  labelBytes[10] = label10;
  labelBytes[11] = label11;
  labelBytes[12] = label12;
  labelBytes[13] = label13;
  labelBytes[14] = label14;
  labelBytes[15] = label15;
  labelBytes[16] = label16;
  labelBytes[17] = label17;
  labelBytes[18] = label18;
  labelBytes[19] = label19;
  labelBytes[20] = label20;
  labelBytes[21] = label21;
  labelBytes[22] = label22;
  labelBytes[23] = label23;
  labelBytes[24] = label24;
  labelBytes[25] = label25;
  labelBytes[26] = label26;
  labelBytes[27] = label27;
  labelBytes[28] = label28;
  labelBytes[29] = label29;
  labelBytes[30] = label30;
  labelBytes[31] = label31;

  component keccak = Keccak(512, 256);

  for (var byte = 0; byte < 32; byte++) {
    for (var bit = 0; bit < 8; bit++) {
      keccak.in[byte * 8 + bit] <== (labelBytes[byte] >> bit) & 1;
      valueBits[byte * 8 + bit] * (valueBits[byte * 8 + bit] - 1) === 0;
      keccak.in[256 + byte * 8 + bit] <== valueBits[byte * 8 + bit];
    }
  }

  var digestBigEndianModFr = 0;
  var digestWeight = 1;
  for (var outByte = 0; outByte < 32; outByte++) {
    for (var outBit = 0; outBit < 8; outBit++) {
      digestBigEndianModFr += keccak.out[(31 - outByte) * 8 + outBit] * digestWeight;
      digestWeight = digestWeight + digestWeight;
    }
  }
  publicSignal === digestBigEndianModFr;
}

template SccpBscFullMessageV1() {
  signal input publicSignals[9];

  signal input messageIdBits[256];
  signal input payloadHashBits[256];
  signal input targetDomainBits[256];
  signal input commitmentRootBits[256];
  signal input finalityHeightBits[256];
  signal input finalityBlockHashBits[256];
  signal input sourceDomainBits[256];
  signal input statementHashBits[256];
  signal input destinationBindingHashBits[256];

  component messageId = SccpBscLabeledKeccakSignal(0x09, 0x1b, 0x17, 0x15, 0xf3, 0x1a, 0xdb, 0xc0, 0x23, 0x93, 0x78, 0xca, 0xf7, 0x7a, 0x43, 0x70, 0xe8, 0x34, 0x85, 0x99, 0x04, 0x8e, 0xc4, 0x5e, 0xfb, 0x20, 0x33, 0x68, 0xdb, 0xcc, 0x50, 0x73);
  for (var messageIdIndex = 0; messageIdIndex < 256; messageIdIndex++) {
    messageId.valueBits[messageIdIndex] <== messageIdBits[messageIdIndex];
  }
  messageId.publicSignal <== publicSignals[0];

  component payloadHash = SccpBscLabeledKeccakSignal(0xd4, 0x0c, 0xf4, 0x31, 0x0a, 0xf2, 0x1a, 0xb1, 0xb3, 0xf1, 0x2d, 0xb2, 0x0d, 0xf9, 0x9a, 0xb8, 0xfe, 0x63, 0xdb, 0xe5, 0x5f, 0xc4, 0x73, 0xe5, 0x45, 0x66, 0x91, 0xc3, 0x9c, 0x18, 0x59, 0xac);
  for (var payloadHashIndex = 0; payloadHashIndex < 256; payloadHashIndex++) {
    payloadHash.valueBits[payloadHashIndex] <== payloadHashBits[payloadHashIndex];
  }
  payloadHash.publicSignal <== publicSignals[1];

  component targetDomain = SccpBscLabeledKeccakSignal(0x5f, 0x7c, 0x13, 0x5f, 0xa3, 0x4a, 0x3f, 0x53, 0xc3, 0x73, 0x3c, 0x64, 0xf1, 0x72, 0xef, 0x8a, 0x63, 0x97, 0x90, 0xcf, 0xe2, 0x40, 0xc9, 0xb4, 0x54, 0x31, 0x1f, 0x8c, 0xbf, 0xe7, 0x4f, 0x96);
  for (var targetDomainIndex = 0; targetDomainIndex < 256; targetDomainIndex++) {
    targetDomain.valueBits[targetDomainIndex] <== targetDomainBits[targetDomainIndex];
  }
  targetDomain.publicSignal <== publicSignals[2];

  component commitmentRoot = SccpBscLabeledKeccakSignal(0xc3, 0xaa, 0x10, 0x56, 0x18, 0x97, 0x74, 0x10, 0x00, 0x7f, 0x32, 0xf4, 0xee, 0xfe, 0x0b, 0x3e, 0xab, 0x17, 0x4a, 0xf6, 0xda, 0xc0, 0xd9, 0x58, 0x29, 0xb9, 0x2e, 0x18, 0x91, 0x2b, 0xfb, 0xe3);
  for (var commitmentRootIndex = 0; commitmentRootIndex < 256; commitmentRootIndex++) {
    commitmentRoot.valueBits[commitmentRootIndex] <== commitmentRootBits[commitmentRootIndex];
  }
  commitmentRoot.publicSignal <== publicSignals[3];

  component finalityHeight = SccpBscLabeledKeccakSignal(0x0d, 0x34, 0x99, 0xb9, 0x35, 0x0c, 0x0a, 0xc6, 0xad, 0xd6, 0xe0, 0x07, 0x67, 0x75, 0xde, 0x67, 0xba, 0xee, 0x79, 0xc5, 0xb6, 0x91, 0xf3, 0xa4, 0xf9, 0x31, 0x7d, 0xcb, 0x97, 0x4d, 0xb5, 0x99);
  for (var finalityHeightIndex = 0; finalityHeightIndex < 256; finalityHeightIndex++) {
    finalityHeight.valueBits[finalityHeightIndex] <== finalityHeightBits[finalityHeightIndex];
  }
  finalityHeight.publicSignal <== publicSignals[4];

  component finalityBlockHash = SccpBscLabeledKeccakSignal(0x1c, 0x5d, 0x46, 0x45, 0xe7, 0x2d, 0x75, 0xc0, 0x15, 0x21, 0x53, 0xa5, 0xfe, 0x86, 0x79, 0xa3, 0xc0, 0xa7, 0xba, 0x6c, 0xfe, 0x3b, 0x91, 0x98, 0x6e, 0x64, 0x7c, 0x4b, 0x26, 0xc1, 0x44, 0xbc);
  for (var finalityBlockHashIndex = 0; finalityBlockHashIndex < 256; finalityBlockHashIndex++) {
    finalityBlockHash.valueBits[finalityBlockHashIndex] <== finalityBlockHashBits[finalityBlockHashIndex];
  }
  finalityBlockHash.publicSignal <== publicSignals[5];

  component sourceDomain = SccpBscLabeledKeccakSignal(0xd0, 0x7e, 0xf0, 0x08, 0x72, 0x59, 0xb4, 0x2a, 0xdc, 0x11, 0x49, 0x7b, 0xe2, 0x75, 0xf4, 0x20, 0x91, 0xc6, 0xef, 0x51, 0xbe, 0xcc, 0xcd, 0x11, 0x3b, 0xe8, 0x60, 0xe1, 0xb4, 0x8a, 0x51, 0x09);
  for (var sourceDomainIndex = 0; sourceDomainIndex < 256; sourceDomainIndex++) {
    sourceDomain.valueBits[sourceDomainIndex] <== sourceDomainBits[sourceDomainIndex];
  }
  sourceDomain.publicSignal <== publicSignals[6];

  component statementHash = SccpBscLabeledKeccakSignal(0xa4, 0x89, 0x56, 0x07, 0xd6, 0x2c, 0x8e, 0x11, 0x63, 0x57, 0xba, 0x7d, 0x10, 0x2e, 0x08, 0xb5, 0x63, 0x68, 0x40, 0xe0, 0x81, 0x6a, 0x60, 0x8f, 0x3a, 0x1f, 0xc9, 0xd0, 0xa1, 0x07, 0x75, 0x69);
  for (var statementHashIndex = 0; statementHashIndex < 256; statementHashIndex++) {
    statementHash.valueBits[statementHashIndex] <== statementHashBits[statementHashIndex];
  }
  statementHash.publicSignal <== publicSignals[7];

  component destinationBindingHash = SccpBscLabeledKeccakSignal(0x09, 0x4c, 0xf2, 0x4d, 0x19, 0x3a, 0xc6, 0x5c, 0x8a, 0x45, 0x01, 0x88, 0xd1, 0x62, 0x82, 0xfb, 0xa8, 0xee, 0x8c, 0x5a, 0x75, 0x39, 0xb7, 0x51, 0x85, 0x7d, 0x23, 0x1f, 0x43, 0x80, 0xc2, 0xdd);
  for (var destinationBindingHashIndex = 0; destinationBindingHashIndex < 256; destinationBindingHashIndex++) {
    destinationBindingHash.valueBits[destinationBindingHashIndex] <== destinationBindingHashBits[destinationBindingHashIndex];
  }
  destinationBindingHash.publicSignal <== publicSignals[8];
}

component main { public [publicSignals] } = SccpBscFullMessageV1();
