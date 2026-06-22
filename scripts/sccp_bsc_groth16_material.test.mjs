import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import {
  BSC_FULL_SCCP_CIRCUIT_PROFILE,
  BSC_GROTH16_PUBLIC_SIGNAL_NAMES,
  BSC_SIGNAL_BINDING_CIRCUIT_PROFILE,
  generateBscSignalBindingCircuitSource,
  main,
  materializeBscGroth16Material,
  snarkjsVerificationKeyToBscVerifierMaterial,
} from "./sccp_bsc_groth16_material.mjs";
import {
  BSC_TESTNET_NETWORK_ID_HEX,
  bscGroth16VerifierKeyHash,
  normalizeVerifierMaterial,
} from "./sccp_bsc_taira_xor_deploy.mjs";

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

test("BSC signal-binding circuit source keeps non-linear constraints and 9 public inputs", () => {
  const source = generateBscSignalBindingCircuitSource();

  assert.match(source, /signal input publicSignals\[9\]/u);
  assert.match(source, /signal input witnessSignals\[9\]/u);
  assert.match(source, /diff\[i\] \* diff\[i\] === 0/u);
  assert.match(source, /component main \{ public \[publicSignals\] \}/u);
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

test("generate command help is exposed through the material CLI", async () => {
  const result = await main(["help"]);

  assert.match(result.help, /sccp_bsc_groth16_material\.mjs generate/u);
  assert.match(result.help, new RegExp(BSC_SIGNAL_BINDING_CIRCUIT_PROFILE, "u"));
});
