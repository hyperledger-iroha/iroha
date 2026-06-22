import assert from "node:assert/strict";
import { createHash, generateKeyPairSync, sign } from "node:crypto";
import { chmod, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import {
  BSC_FULL_SCCP_CIRCUIT_PROFILE,
  BSC_GROTH16_CIRCUIT_SECURITY_ATTESTATION_SCHEMA,
  BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA,
  BSC_GROTH16_PUBLIC_SIGNAL_NAMES,
  BSC_GROTH16_REPRODUCIBLE_BUILD_ATTESTATION_SCHEMA,
  BSC_GROTH16_SEMANTIC_ATTESTATION_SCHEMA,
  BSC_GROTH16_TRUSTED_SETUP_ATTESTATION_SCHEMA,
  BSC_SIGNAL_BINDING_CIRCUIT_PROFILE,
  generateBscFullMessageCircuitSource,
  generateBscSignalBindingCircuitSource,
  main,
  materializeBscGroth16Material,
  snarkjsVerificationKeyToBscVerifierMaterial,
} from "./sccp_bsc_groth16_material.mjs";
import {
  BSC_TESTNET_CHAIN_ID_HEX,
  BSC_TESTNET_NETWORK_ID_HEX,
  bscGroth16VerifierKeyHash,
  normalizeVerifierMaterial,
} from "./sccp_bsc_taira_xor_deploy.mjs";

const sha256Hex = (bytes) =>
  `0x${createHash("sha256").update(bytes).digest("hex")}`;

const TEST_ATTESTATION_SIGNER = generateKeyPairSync("ed25519");
const TEST_ATTESTATION_PUBLIC_KEY_PEM = TEST_ATTESTATION_SIGNER.publicKey.export({
  type: "spki",
  format: "pem",
});
const TEST_ATTESTATION_SIGNER_FINGERPRINT = sha256Hex(
  TEST_ATTESTATION_SIGNER.publicKey.export({
    type: "spki",
    format: "der",
  }),
);
const SECURITY_ATTESTATION_SIGNER = generateKeyPairSync("ed25519");
const SECURITY_ATTESTATION_PUBLIC_KEY_PEM =
  SECURITY_ATTESTATION_SIGNER.publicKey.export({
    type: "spki",
    format: "pem",
  });
const SECURITY_ATTESTATION_SIGNER_FINGERPRINT = sha256Hex(
  SECURITY_ATTESTATION_SIGNER.publicKey.export({
    type: "spki",
    format: "der",
  }),
);
const SETUP_ATTESTATION_SIGNER = generateKeyPairSync("ed25519");
const SETUP_ATTESTATION_PUBLIC_KEY_PEM =
  SETUP_ATTESTATION_SIGNER.publicKey.export({
    type: "spki",
    format: "pem",
  });
const SETUP_ATTESTATION_SIGNER_FINGERPRINT = sha256Hex(
  SETUP_ATTESTATION_SIGNER.publicKey.export({
    type: "spki",
    format: "der",
  }),
);
const REPRODUCIBLE_ATTESTATION_SIGNER = generateKeyPairSync("ed25519");
const REPRODUCIBLE_ATTESTATION_PUBLIC_KEY_PEM =
  REPRODUCIBLE_ATTESTATION_SIGNER.publicKey.export({
    type: "spki",
    format: "pem",
  });
const REPRODUCIBLE_ATTESTATION_SIGNER_FINGERPRINT = sha256Hex(
  REPRODUCIBLE_ATTESTATION_SIGNER.publicKey.export({
    type: "spki",
    format: "der",
  }),
);
const TRUSTED_ATTESTATION_SIGNER_FINGERPRINTS = Object.freeze([
  TEST_ATTESTATION_SIGNER_FINGERPRINT,
  SECURITY_ATTESTATION_SIGNER_FINGERPRINT,
  SETUP_ATTESTATION_SIGNER_FINGERPRINT,
  REPRODUCIBLE_ATTESTATION_SIGNER_FINGERPRINT,
]);
const UNTRUSTED_ATTESTATION_SIGNER = generateKeyPairSync("ed25519");
const UNTRUSTED_ATTESTATION_PUBLIC_KEY_PEM =
  UNTRUSTED_ATTESTATION_SIGNER.publicKey.export({
    type: "spki",
    format: "pem",
  });
const UNTRUSTED_ATTESTATION_SIGNER_FINGERPRINT = sha256Hex(
  UNTRUSTED_ATTESTATION_SIGNER.publicKey.export({
    type: "spki",
    format: "der",
  }),
);

function canonicalJson(value) {
  if (value === null) return "null";
  if (typeof value === "string") return JSON.stringify(value);
  if (typeof value === "number") return JSON.stringify(value);
  if (typeof value === "boolean") return value ? "true" : "false";
  if (Array.isArray(value)) {
    return `[${value.map((entry) => canonicalJson(entry)).join(",")}]`;
  }
  return `{${Object.keys(value)
    .sort()
    .map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`)
    .join(",")}}`;
}

function signAttestationRecord(
  record,
  {
    privateKey = TEST_ATTESTATION_SIGNER.privateKey,
    publicKeyPem = TEST_ATTESTATION_PUBLIC_KEY_PEM,
    signerFingerprint = TEST_ATTESTATION_SIGNER_FINGERPRINT,
    algorithm = "ed25519",
    schema = BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA,
    signedPayloadSha256 = null,
  } = {},
) {
  const payload = Buffer.from(canonicalJson(record), "utf8");
  return {
    ...record,
    signature: {
      schema,
      algorithm,
      signerFingerprint,
      publicKeyPem,
      signedPayloadSha256: signedPayloadSha256 ?? sha256Hex(payload),
      signature: sign(null, payload, privateKey).toString("base64"),
    },
  };
}

const trustedSignerOption = () => ({
  "trusted-attestation-signer": TRUSTED_ATTESTATION_SIGNER_FINGERPRINTS.join(","),
});

const defaultAttestationSigning = () => ({
  semantic: {
    privateKey: TEST_ATTESTATION_SIGNER.privateKey,
    publicKeyPem: TEST_ATTESTATION_PUBLIC_KEY_PEM,
    signerFingerprint: TEST_ATTESTATION_SIGNER_FINGERPRINT,
  },
  security: {
    privateKey: SECURITY_ATTESTATION_SIGNER.privateKey,
    publicKeyPem: SECURITY_ATTESTATION_PUBLIC_KEY_PEM,
    signerFingerprint: SECURITY_ATTESTATION_SIGNER_FINGERPRINT,
  },
  setup: {
    privateKey: SETUP_ATTESTATION_SIGNER.privateKey,
    publicKeyPem: SETUP_ATTESTATION_PUBLIC_KEY_PEM,
    signerFingerprint: SETUP_ATTESTATION_SIGNER_FINGERPRINT,
  },
  reproducible: {
    privateKey: REPRODUCIBLE_ATTESTATION_SIGNER.privateKey,
    publicKeyPem: REPRODUCIBLE_ATTESTATION_PUBLIC_KEY_PEM,
    signerFingerprint: REPRODUCIBLE_ATTESTATION_SIGNER_FINGERPRINT,
  },
});

const VALID_G1 = [
  "1368015179489954701390400359078579693043519447331113978918064868415326638035",
  "9918110051302171585080402603319702774565515993150576347155970296011118125764",
  "1",
];
const VALID_G1_ALT = [
  "3353031288059533942658390886683067124040920775575537747144343083137631628272",
  "19321533766552368860946552437480515441416830039777911637913418824951667761761",
  "1",
];
const SOLIDITY_G2_GENERATOR = [
  "10857046999023057135944570762232829481370756359578518086990519993285655852781",
  "11559732032986387107991004021392285783925812861821192530917403151452391805634",
  "8495653923123431417604973247489272438418190587263600148770280649306958101930",
  "4082367875863433681332203403145435568316851327593401208105741076214120093531",
];
const SNARKJS_G2_GENERATOR = [
  [SOLIDITY_G2_GENERATOR[0], SOLIDITY_G2_GENERATOR[1]],
  [SOLIDITY_G2_GENERATOR[2], SOLIDITY_G2_GENERATOR[3]],
  ["1", "0"],
];

function verificationKey(overrides = {}) {
  return {
    protocol: "groth16",
    curve: "bn128",
    nPublic: 9,
    vk_alpha_1: VALID_G1,
    vk_beta_2: SNARKJS_G2_GENERATOR,
    vk_gamma_2: SNARKJS_G2_GENERATOR,
    vk_delta_2: SNARKJS_G2_GENERATOR,
    IC: Array.from({ length: 10 }, (_, index) =>
      index % 2 === 0 ? VALID_G1 : VALID_G1_ALT,
    ),
    ...overrides,
  };
}

function u32le(value) {
  const out = Buffer.alloc(4);
  out.writeUInt32LE(value, 0);
  return out;
}

function u64le(value) {
  const out = Buffer.alloc(8);
  out.writeUInt32LE(value >>> 0, 0);
  out.writeUInt32LE(Math.floor(value / 0x100000000), 4);
  return out;
}

function snarkjsMaterialBytes(magic, sectionCount = 3, sizeBytes = 70 * 1024) {
  const headerBytes = 12 + sectionCount * 12;
  const payloadBytes = sizeBytes - headerBytes;
  const sectionSize = Math.floor(payloadBytes / sectionCount);
  const parts = [Buffer.from(magic, "ascii"), u32le(1), u32le(sectionCount)];
  let remaining = payloadBytes;
  for (let index = 0; index < sectionCount; index += 1) {
    const currentSize =
      index === sectionCount - 1 ? remaining : sectionSize;
    remaining -= currentSize;
    const payload = Buffer.alloc(currentSize);
    for (let cursor = 0; cursor < payload.length; cursor += 1) {
      payload[cursor] = (index * 31 + cursor * 17 + 19) & 0xff;
    }
    parts.push(u32le(index + 1), u64le(currentSize), payload);
  }
  return Buffer.concat(parts);
}

function verifierKeyBytesFor(input) {
  return Buffer.from(`${JSON.stringify(input, null, 2)}\n`);
}

function attestationBase({
  schema,
  context,
  extra = {},
}) {
  return {
    schema,
    routeId: "taira_bsc_xor",
    assetKey: "xor",
    bscNetwork: "testnet",
    chain: "bsc-testnet",
    chainIdHex: BSC_TESTNET_CHAIN_ID_HEX,
    networkIdHex: BSC_TESTNET_NETWORK_ID_HEX,
    circuitProfile: BSC_FULL_SCCP_CIRCUIT_PROFILE,
    publicInputCount: 9,
    publicSignalNames: [...BSC_GROTH16_PUBLIC_SIGNAL_NAMES],
    verifierKeyHash: context.verifierKeyHash,
    circuitSourceSha256: context.circuitSourceSha256,
    r1csSha256: context.r1csSha256,
    provingKeySha256: context.provingKeySha256,
    snarkjsVerificationKeySha256: context.snarkjsVerificationKeySha256,
    bscVerifierKeySha256: context.bscVerifierKeySha256,
    ...extra,
  };
}

async function writeJson(pathName, value) {
  await writeFile(pathName, `${JSON.stringify(value, null, 2)}\n`);
}

async function writeBoundAttestations(root, context, overrides = {}) {
  const semantic = join(root, "semantic.json");
  const security = join(root, "security.json");
  const setup = join(root, "setup.json");
  const reproducible = join(root, "reproducible.json");
  const signing = overrides.signing
    ? {
        semantic: overrides.signing,
        security: overrides.signing,
        setup: overrides.signing,
        reproducible: overrides.signing,
      }
    : {
        ...defaultAttestationSigning(),
        ...(overrides.signingByRole ?? {}),
      };
  await writeJson(
    semantic,
    signAttestationRecord(
      attestationBase({
        schema: BSC_GROTH16_SEMANTIC_ATTESTATION_SCHEMA,
        context,
        extra: {
          fullSccpMessageSemantics: true,
          sourceFinalitySemantics: true,
          destinationBindingSemantics: true,
          publicSignalDerivationSemantics: true,
          negativeCaseCoverage: true,
          ...(overrides.semantic ?? {}),
        },
      }),
      signing.semantic,
    ),
  );
  await writeJson(
    security,
    signAttestationRecord(
      attestationBase({
        schema: BSC_GROTH16_CIRCUIT_SECURITY_ATTESTATION_SCHEMA,
        context,
        extra: {
          auditResult: "pass",
          approved: true,
          criticalFindings: 0,
          highFindings: 0,
          unresolvedFindings: 0,
          ...(overrides.security ?? {}),
        },
      }),
      signing.security,
    ),
  );
  await writeJson(
    setup,
    signAttestationRecord(
      attestationBase({
        schema: BSC_GROTH16_TRUSTED_SETUP_ATTESTATION_SCHEMA,
        context,
        extra: {
          ceremonyResult: "pass",
          localSingleContributor: false,
          minimumContributors: 3,
          toxicWasteDestroyed: true,
          contributionTranscriptSha256: `0x${"ab".repeat(32)}`,
          ...(overrides.setup ?? {}),
        },
      }),
      signing.setup,
    ),
  );
  await writeJson(
    reproducible,
    signAttestationRecord(
      attestationBase({
        schema: BSC_GROTH16_REPRODUCIBLE_BUILD_ATTESTATION_SCHEMA,
        context,
        extra: {
          reproducible: true,
          independentRebuilders: 2,
          buildTranscriptSha256: `0x${"cd".repeat(32)}`,
          ...(overrides.reproducible ?? {}),
        },
      }),
      signing.reproducible,
    ),
  );
  return { semantic, security, setup, reproducible };
}

async function writeMaterialInputs(root) {
  const r1csBytes = snarkjsMaterialBytes("r1cs", 3);
  const zkeyBytes = snarkjsMaterialBytes("zkey", 10, 96 * 1024);
  const snarkjsKey = verificationKey();
  const bscVerifierMaterial = snarkjsVerificationKeyToBscVerifierMaterial(
    snarkjsKey,
    { bscNetwork: "testnet" },
  );
  const r1cs = join(root, "full.r1cs");
  const zkey = join(root, "full.zkey");
  const verificationKeyPath = join(root, "verification_key.json");
  const circuitSource = join(root, "full.circom");
  const circuitSourceText = generateBscFullMessageCircuitSource();
  await writeFile(r1cs, r1csBytes);
  await writeFile(zkey, zkeyBytes);
  await writeJson(verificationKeyPath, snarkjsKey);
  await writeFile(circuitSource, circuitSourceText);
  const bscVerifierKeySha256 = sha256Hex(verifierKeyBytesFor(bscVerifierMaterial));
  return {
    r1cs,
    zkey,
    verificationKeyPath,
    circuitSource,
    context: {
      verifierKeyHash: bscVerifierMaterial.verifierKeyHash,
      circuitSourceSha256: sha256Hex(Buffer.from(circuitSourceText)),
      r1csSha256: sha256Hex(r1csBytes),
      provingKeySha256: sha256Hex(zkeyBytes),
      snarkjsVerificationKeySha256: sha256Hex(
        Buffer.from(`${JSON.stringify(snarkjsKey, null, 2)}\n`),
      ),
      bscVerifierKeySha256,
    },
  };
}

async function writeSnarkjsStub(
  root,
  verificationKeyPath,
  { publicInputCount = 9, constraintCount = 8192 } = {},
) {
  const stubPath = join(root, "snarkjs-stub.cjs");
  await writeFile(
    stubPath,
    `#!/usr/bin/env node
const { readFileSync, writeFileSync } = require("node:fs");
const args = process.argv.slice(2);
if (args[0] === "r1cs" && args[1] === "info" && args[2]) {
  process.stdout.write("# of Constraints: ${constraintCount}\\n# of Public Inputs: ${publicInputCount}\\n");
  process.exit(0);
}
if (args[0] === "zkey" && args[1] === "export" && args[2] === "verificationkey" && args[3] && args[4]) {
  writeFileSync(args[4], readFileSync(${JSON.stringify(verificationKeyPath)}));
  process.exit(0);
}
process.stderr.write("unexpected snarkjs stub invocation: " + args.join(" ") + "\\n");
process.exit(2);
`,
    { mode: 0o755 },
  );
  await chmod(stubPath, 0o755);
  return stubPath;
}

test("BSC Groth16 material converter maps SnarkJS verifier key to Solidity constructor order", () => {
  const material = snarkjsVerificationKeyToBscVerifierMaterial(
    verificationKey(),
    { bscNetwork: "testnet" },
  );

  assert.equal(material.routeId, "taira_bsc_xor");
  assert.equal(material.bscNetwork, "testnet");
  assert.equal(material.networkIdHex, BSC_TESTNET_NETWORK_ID_HEX);
  assert.deepEqual(material.beta2, SOLIDITY_G2_GENERATOR);
  assert.deepEqual(material.gamma2, SOLIDITY_G2_GENERATOR);
  assert.deepEqual(material.delta2, SOLIDITY_G2_GENERATOR);
  assert.equal(material.ic.length, 20);
  assert.equal(material.publicInputCount, 9);
  assert.deepEqual(material.publicSignalNames, BSC_GROTH16_PUBLIC_SIGNAL_NAMES);
  assert.equal(material.verifierKeyHash, bscGroth16VerifierKeyHash(material));
  assert.equal(
    normalizeVerifierMaterial(material).expectedVerifierKeyHash,
    material.verifierKeyHash,
  );
});

test("BSC Groth16 material converter rejects verifier keys with wrong public input count", () => {
  assert.throws(
    () =>
      snarkjsVerificationKeyToBscVerifierMaterial(
        verificationKey({ nPublic: 8 }),
      ),
    /nPublic must be 9/u,
  );
  assert.throws(
    () =>
      snarkjsVerificationKeyToBscVerifierMaterial(
        verificationKey({ IC: Array.from({ length: 9 }, () => VALID_G1) }),
      ),
    /IC must contain exactly 10 G1 points/u,
  );
});

test("BSC Groth16 material converter rejects adversarial verifier keys", () => {
  assert.throws(
    () =>
      snarkjsVerificationKeyToBscVerifierMaterial(
        verificationKey({ protocol: "plonk" }),
      ),
    /protocol must be groth16/u,
  );
  assert.throws(
    () =>
      snarkjsVerificationKeyToBscVerifierMaterial(
        verificationKey({ curve: "bls12381" }),
      ),
    /curve must be bn128\/bn254/u,
  );
  assert.throws(
    () =>
      snarkjsVerificationKeyToBscVerifierMaterial(
        verificationKey({
          vk_beta_2: [
            ["1", "2"],
            ["3", "4"],
            ["1", "0"],
          ],
        }),
      ),
    /beta2 must be on the BN254 G2 twist curve/u,
  );
});

test("BSC signal-binding circuit source keeps non-linear constraints and 9 public inputs", () => {
  const source = generateBscSignalBindingCircuitSource();

  assert.match(source, /signal input publicSignals\[9\]/u);
  assert.match(source, /signal input witnessSignals\[9\]/u);
  assert.match(source, /diff\[i\] \* diff\[i\] === 0/u);
  assert.match(source, /component main \{ public \[publicSignals\] \}/u);
});

test("BSC full message circuit source exposes Keccak-derived production signals", () => {
  const source = generateBscFullMessageCircuitSource();

  assert.match(source, /template SccpBscFullMessageV1/u);
  assert.match(source, /template SccpBscLabeledKeccakSignal/u);
  assert.match(source, /Keccak\(512, 256\)/u);
  assert.match(source, /circomlib\/circuits\/gates\.circom/u);
  assert.match(source, /circomlib\/circuits\/sha256\/xor3\.circom/u);
  assert.match(source, /circomlib\/circuits\/sha256\/shift\.circom/u);
  assert.match(source, /@electron-labs\/keccak-circom\/circuits\/keccak\.circom/u);
  assert.match(source, /publicSignal === digestBigEndianModFr/u);
  assert.match(source, /valueBits\[byte \* 8 \+ bit\] \* \(valueBits\[byte \* 8 \+ bit\] - 1\) === 0/u);
  for (let index = 0; index < 9; index += 1) {
    assert.match(source, new RegExp(`publicSignals\\[${index}\\]`, "u"));
  }
  assert.match(source, /message_id/u);
  assert.match(source, /payload_hash/u);
  assert.match(source, /target_domain/u);
  assert.match(source, /commitment_root/u);
  assert.match(source, /finality_height/u);
  assert.match(source, /finality_block_hash/u);
  assert.match(source, /source_domain/u);
  assert.match(source, /statement_hash/u);
  assert.match(source, /destination_binding_hash/u);
  assert.doesNotMatch(source, /SccpBscSignalBindingV1/u);
  assert.doesNotMatch(source, /witnessSignals/u);
  assert.doesNotMatch(source, /must connect|Implementations must wire|fixture fallback/u);
});

test("materialize writes verifier material but fails closed without production attestations", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-material-"));
  try {
    const r1cs = join(root, "candidate.r1cs");
    const zkey = join(root, "candidate.zkey");
    const verificationKeyPath = join(root, "verification_key.json");
    await writeFile(r1cs, Buffer.from("r1cs\x01\x00\x00\x00", "binary"));
    await writeFile(zkey, Buffer.from("zkey\x01\x00\x00\x00", "binary"));
    await writeFile(
      verificationKeyPath,
      `${JSON.stringify(verificationKey(), null, 2)}\n`,
    );

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      r1cs,
      zkey,
      "snarkjs-verifier-key": verificationKeyPath,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /missing semantic SCCP circuit attestation/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /missing trusted setup ceremony attestation/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /R1CS must be at least 65536 bytes/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /zkey must be at least 65536 bytes/u,
    );
    const manifest = JSON.parse(await readFile(result.manifest, "utf8"));
    assert.equal(manifest.circuitProfile, BSC_FULL_SCCP_CIRCUIT_PROFILE);
    assert.equal(manifest.productionReady, false);
    const verifier = JSON.parse(await readFile(result.verifierKey, "utf8"));
    assert.equal(verifier.verifierKeyHash, result.verifierKeyHash);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("generate refuses local setup unless explicitly constrained to testnet candidates", async () => {
  await assert.rejects(
    () =>
      main([
        "generate",
        "--bsc-network",
        "testnet",
        "--create-local-ptau-power",
        "8",
      ]),
    /requires --allow-local-testnet-setup true/u,
  );
  await assert.rejects(
    () =>
      main([
        "generate",
        "--bsc-network",
        "mainnet",
        "--create-local-ptau-power",
        "8",
        "--allow-local-testnet-setup",
        "true",
      ]),
    /only allowed for testnet candidates/u,
  );
});

test("generate refuses full-message material without an external circuit source", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-full-generate-"));
  try {
    const ptau = join(root, "phase2.ptau");
    await writeFile(ptau, Buffer.from("ptau"));

    await assert.rejects(
      () =>
        main([
          "generate",
          "--bsc-network",
          "testnet",
          "--ptau",
          ptau,
          "--circuit-profile",
          BSC_FULL_SCCP_CIRCUIT_PROFILE,
          "--out-dir",
          join(root, "out"),
        ]),
      /requires --circuit-source/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses forged or unsafe production attestations", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-bad-attest-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const attestations = await writeBoundAttestations(root, inputs.context, {
      semantic: {
        r1csSha256: `0x${"12".repeat(32)}`,
      },
      setup: {
        localSingleContributor: true,
        minimumContributors: 1,
      },
    });

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "circuit-source": inputs.circuitSource,
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /semantic SCCP circuit r1csSha256 must match/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /trusted setup localSingleContributor must be false/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /trusted setup minimumContributors must be at least 2/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses attestations repackaged across chain ids or circuit source", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-drift-attest-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const attestations = await writeBoundAttestations(root, inputs.context, {
      semantic: {
        chainIdHex: "0x38",
      },
      reproducible: {
        circuitSourceSha256: `0x${"34".repeat(32)}`,
      },
    });

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "circuit-source": inputs.circuitSource,
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /semantic SCCP circuit chainIdHex must be 0x61/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /reproducible build circuitSourceSha256 must match/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses unsigned, untrusted, or tampered attestations", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-signed-attest-"));
  try {
    const runCase = async (name, configure, extraOptions = trustedSignerOption()) => {
      const caseRoot = join(root, name);
      await mkdir(caseRoot, { recursive: true });
      const inputs = await writeMaterialInputs(caseRoot);
      const attestations = await writeBoundAttestations(caseRoot, inputs.context);
      await configure?.({ root: caseRoot, inputs, attestations });
      const snarkjsStub = await writeSnarkjsStub(
        caseRoot,
        inputs.verificationKeyPath,
      );
      return materializeBscGroth16Material({
        "bsc-network": "testnet",
        ...extraOptions,
        r1cs: inputs.r1cs,
        zkey: inputs.zkey,
        "snarkjs-verifier-key": inputs.verificationKeyPath,
        "snarkjs-bin": snarkjsStub,
        "circuit-source": inputs.circuitSource,
        "semantic-attestation": attestations.semantic,
        "circuit-security-attestation": attestations.security,
        "trusted-setup-attestation": attestations.setup,
        "reproducible-build-attestation": attestations.reproducible,
        "out-dir": join(caseRoot, "out"),
      });
    };

    const noTrustedSigner = await runCase(
      "no-trusted-signer",
      null,
      {},
    );
    assert.equal(noTrustedSigner.productionReady, false);
    assert.match(
      noTrustedSigner.productionBlockers.join("\n"),
      /semantic SCCP circuit attestation trusted attestation signer fingerprint is required/u,
    );

    const unsigned = await runCase("unsigned", async ({ attestations }) => {
      const semantic = JSON.parse(await readFile(attestations.semantic, "utf8"));
      delete semantic.signature;
      await writeJson(attestations.semantic, semantic);
    });
    assert.equal(unsigned.productionReady, false);
    assert.match(
      unsigned.productionBlockers.join("\n"),
      /semantic SCCP circuit attestation signature is required/u,
    );

    const untrusted = await runCase("untrusted", async ({ root, inputs, attestations }) => {
      await writeBoundAttestations(root, inputs.context, {
        signing: {
          privateKey: UNTRUSTED_ATTESTATION_SIGNER.privateKey,
          publicKeyPem: UNTRUSTED_ATTESTATION_PUBLIC_KEY_PEM,
          signerFingerprint: UNTRUSTED_ATTESTATION_SIGNER_FINGERPRINT,
        },
      });
      assert.equal(typeof attestations.semantic, "string");
    });
    assert.equal(untrusted.productionReady, false);
    assert.match(
      untrusted.productionBlockers.join("\n"),
      /semantic SCCP circuit attestation signature signerFingerprint is not trusted/u,
    );

    const tampered = await runCase("tampered", async ({ attestations }) => {
      const semantic = JSON.parse(await readFile(attestations.semantic, "utf8"));
      semantic.reviewedBy = "post-sign tamper";
      await writeJson(attestations.semantic, semantic);
    });
    assert.equal(tampered.productionReady, false);
    assert.match(
      tampered.productionBlockers.join("\n"),
      /semantic SCCP circuit attestation signature signedPayloadSha256 must match attestation body/u,
    );
    assert.match(
      tampered.productionBlockers.join("\n"),
      /semantic SCCP circuit attestation detached signature verification failed/u,
    );

    const badPayloadHash = await runCase("bad-payload-hash", async ({ root, inputs }) => {
      await writeBoundAttestations(root, inputs.context, {
        signing: {
          signedPayloadSha256: `0x${"42".repeat(32)}`,
        },
      });
    });
    assert.equal(badPayloadHash.productionReady, false);
    assert.match(
      badPayloadHash.productionBlockers.join("\n"),
      /signature signedPayloadSha256 must match attestation body/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses production attestations that reuse a signer across roles", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-reused-signer-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const attestations = await writeBoundAttestations(root, inputs.context, {
      signing: defaultAttestationSigning().semantic,
    });
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath);

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /production Groth16 attestation signers must be role-separated/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize rejects secret-like attestation contents before manifest write", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-secret-attest-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const attestations = await writeBoundAttestations(root, inputs.context);
    const semantic = JSON.parse(await readFile(attestations.semantic, "utf8"));
    semantic.privateKey = `0x${"11".repeat(32)}`;
    await writeJson(attestations.semantic, semantic);

    await assert.rejects(
      () =>
        materializeBscGroth16Material({
          "bsc-network": "testnet",
          ...trustedSignerOption(),
          r1cs: inputs.r1cs,
          zkey: inputs.zkey,
          "snarkjs-verifier-key": inputs.verificationKeyPath,
          "circuit-source": inputs.circuitSource,
          "semantic-attestation": attestations.semantic,
          "circuit-security-attestation": attestations.security,
          "trusted-setup-attestation": attestations.setup,
          "reproducible-build-attestation": attestations.reproducible,
          "out-dir": join(root, "out"),
        }),
      /semantic SCCP circuit attestation\.privateKey must not contain/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses signal-binding circuits repackaged as full material", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-signal-repack-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const signalBindingSource = generateBscSignalBindingCircuitSource();
    await writeFile(inputs.circuitSource, signalBindingSource);
    const attestations = await writeBoundAttestations(root, {
      ...inputs.context,
      circuitSourceSha256: sha256Hex(Buffer.from(signalBindingSource)),
    });
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath);

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /must not use the signal-binding fixture circuit/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses placeholder full-message circuit sources", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-placeholder-source-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const placeholderSource = `pragma circom 2.1.6;
include "circomlib/circuits/bitify.circom";
include "@electron-labs/keccak-circom/circuits/keccak.circom";
template SccpBscFullMessageV1() {
  signal input publicSignals[9];
  component signalKeccak[9];
  for (var i = 0; i < 9; i++) {
    signalKeccak[i] = Keccak(512, 256);
  }
  // Implementations must wire label constants and publicSignals here.
}
component main { public [publicSignals] } = SccpBscFullMessageV1();
`;
    await writeFile(inputs.circuitSource, placeholderSource);
    const attestations = await writeBoundAttestations(root, {
      ...inputs.context,
      circuitSourceSha256: sha256Hex(Buffer.from(placeholderSource)),
    });
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath);

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /unresolved scaffold placeholders/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /constrain all 9 publicSignals entries/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /bind all 9 Solidity signal labels/u,
    );
    const manifest = JSON.parse(await readFile(result.manifest, "utf8"));
    assert.equal(manifest.selfChecks.circuitSource.unresolvedPlaceholders, true);
    assert.equal(manifest.selfChecks.circuitSource.publicSignalConstraintCount, 0);
    assert.equal(manifest.selfChecks.circuitSource.labelBindingCount, 0);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize marks full material ready only with bound attestations", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-good-attest-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const attestations = await writeBoundAttestations(root, inputs.context);
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath);

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, true);
    assert.deepEqual(result.productionBlockers, []);
    const manifest = JSON.parse(await readFile(result.manifest, "utf8"));
    assert.equal(manifest.productionReady, true);
    assert.equal(manifest.selfChecks.snarkjs.r1csInfo, true);
    assert.equal(manifest.selfChecks.snarkjs.r1csPublicInputCount, 9);
    assert.equal(manifest.selfChecks.snarkjs.r1csConstraintCount, 8192);
    assert.equal(manifest.selfChecks.snarkjs.zkeyVerificationKeyExport, true);
    assert.equal(manifest.selfChecks.snarkjs.verifierKeyHashMatches, true);
    assert.deepEqual(manifest.selfChecks.circuitSource, {
      fullMessageCircuit: true,
      signalBindingFixture: false,
      unresolvedPlaceholders: false,
      keccakPublicSignalDerivation: true,
      digestReductionModuloScalarField: true,
      valueBitBooleanConstraints: true,
      publicSignalConstraintCount: 9,
      labelBindingCount: 9,
    });
    assert.equal(
      manifest.selfChecks.snarkjs.exportedVerifierKeyHash,
      result.verifierKeyHash,
    );
    assert.deepEqual(manifest.attestationTrustPolicy, {
      signatureSchema: BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA,
      requiredAlgorithm: "ed25519",
      trustedSignerFingerprints: TRUSTED_ATTESTATION_SIGNER_FINGERPRINTS,
    });
    assert.equal(
      manifest.attestations.semanticSccpCircuit.schema,
      BSC_GROTH16_SEMANTIC_ATTESTATION_SCHEMA,
    );
    const {
      signature: _semanticSignature,
      signatures: _semanticSignatures,
      ...semanticSignedBody
    } = JSON.parse(await readFile(attestations.semantic, "utf8"));
    assert.deepEqual(manifest.attestations.semanticSccpCircuit.signature, {
      verified: true,
      algorithm: "ed25519",
      signerFingerprint: TEST_ATTESTATION_SIGNER_FINGERPRINT,
      signedPayloadSha256: sha256Hex(
        Buffer.from(canonicalJson(semanticSignedBody)),
      ),
    });
    assert.equal(
      Object.prototype.hasOwnProperty.call(
        manifest.attestations.semanticSccpCircuit,
        "record",
      ),
      false,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses full circuit material with wrong R1CS signal counts", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-r1cs-counts-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const attestations = await writeBoundAttestations(root, inputs.context);
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath, {
      publicInputCount: 8,
      constraintCount: 128,
    });

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /public input count must be 9/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /constraint count must be at least 4096/u,
    );
    const manifest = JSON.parse(await readFile(result.manifest, "utf8"));
    assert.equal(manifest.selfChecks.snarkjs.r1csInfo, true);
    assert.equal(manifest.selfChecks.snarkjs.r1csPublicInputCount, 8);
    assert.equal(manifest.selfChecks.snarkjs.r1csConstraintCount, 128);
    assert.equal(manifest.selfChecks.snarkjs.verifierKeyHashMatches, true);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses zkeys that export a different verifier key", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-zkey-drift-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const attestations = await writeBoundAttestations(root, inputs.context);
    const alternateVerificationKeyPath = join(root, "alternate_verification_key.json");
    await writeJson(
      alternateVerificationKeyPath,
      verificationKey({
        IC: Array.from({ length: 10 }, (_, index) =>
          index % 2 === 0 ? VALID_G1_ALT : VALID_G1,
        ),
      }),
    );
    const snarkjsStub = await writeSnarkjsStub(
      root,
      alternateVerificationKeyPath,
    );

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /zkey export hash mismatch/u,
    );
    const manifest = JSON.parse(await readFile(result.manifest, "utf8"));
    assert.equal(manifest.selfChecks.snarkjs.r1csInfo, true);
    assert.equal(manifest.selfChecks.snarkjs.zkeyVerificationKeyExport, true);
    assert.equal(manifest.selfChecks.snarkjs.verifierKeyHashMatches, false);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("generate command help is exposed through the material CLI", async () => {
  const result = await main(["help"]);

  assert.match(result.help, /sccp_bsc_groth16_material\.mjs generate/u);
  assert.match(result.help, new RegExp(BSC_SIGNAL_BINDING_CIRCUIT_PROFILE, "u"));
});
