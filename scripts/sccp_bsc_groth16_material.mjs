#!/usr/bin/env node
// Purpose: generate and inspect public BSC SCCP Groth16 circuit/proving
// material without writing operator credentials. Locally generated setup
// output is marked as a production candidate only; production-ready status
// requires externally audited circuit semantics and ceremony evidence.
import { spawn } from "node:child_process";
import {
  createHash,
  createPublicKey,
  randomBytes,
  verify as verifyDetachedSignature,
} from "node:crypto";
import {
  copyFile,
  lstat,
  mkdtemp,
  mkdir,
  readFile,
  rename,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import {
  basename,
  dirname,
  isAbsolute,
  join,
  relative,
  resolve,
} from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import {
  ASSET_KEY,
  BSC_EVM_GROTH16_BACKEND,
  BSC_NETWORK_PROFILES,
  DEFAULT_NATIVE_EVM_PROVER_ARTIFACT_ROOT,
  ROUTE_ID,
  SCCP_DOMAIN_BSC,
  SCCP_DOMAIN_SORA,
  SCCP_PROOF_FAMILY_STARK_FRI,
  bscGroth16VerifierKeyHash,
  normalizeBscNetworkProfile,
  normalizeHex32,
  normalizeVerifierMaterial,
  unsafeSecretReason,
} from "./sccp_bsc_taira_xor_deploy.mjs";

const SCRIPT_PATH = fileURLToPath(import.meta.url);
const REPO_ROOT = resolve(dirname(SCRIPT_PATH), "..");
const textEncoder = new TextEncoder();

export const BSC_GROTH16_MATERIAL_MANIFEST_SCHEMA =
  "iroha-sccp-bsc-groth16-material-manifest/v1";
export const BSC_GROTH16_VERIFIER_KEY_SCHEMA =
  "iroha-sccp-bsc-groth16-verifier-key/v1";
export const BSC_GROTH16_SEMANTIC_ATTESTATION_SCHEMA =
  "iroha-sccp-bsc-groth16-semantic-circuit-attestation/v1";
export const BSC_GROTH16_CIRCUIT_SECURITY_ATTESTATION_SCHEMA =
  "iroha-sccp-bsc-groth16-circuit-security-attestation/v1";
export const BSC_GROTH16_TRUSTED_SETUP_ATTESTATION_SCHEMA =
  "iroha-sccp-bsc-groth16-trusted-setup-attestation/v1";
export const BSC_GROTH16_REPRODUCIBLE_BUILD_ATTESTATION_SCHEMA =
  "iroha-sccp-bsc-groth16-reproducible-build-attestation/v1";
export const BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA =
  "iroha-sccp-bsc-groth16-attestation-signature/v1";
export const BSC_SIGNAL_BINDING_CIRCUIT_PROFILE =
  "sccp-bsc-signal-binding-v1";
export const BSC_FULL_SCCP_CIRCUIT_PROFILE = "sccp-bsc-full-message-v1";
export const BSC_GROTH16_PUBLIC_SIGNAL_NAMES = Object.freeze([
  "message_id",
  "payload_hash",
  "target_domain",
  "commitment_root",
  "finality_height",
  "finality_block_hash",
  "source_domain",
  "statement_hash",
  "destination_binding_hash",
]);
export const BSC_GROTH16_PUBLIC_SIGNAL_LABEL_HASHES = Object.freeze([
  "0x091b1715f31adbc0239378caf77a4370e8348599048ec45efb203368dbcc5073",
  "0xd40cf4310af21ab1b3f12db20df99ab8fe63dbe55fc473e5456691c39c1859ac",
  "0x5f7c135fa34a3f53c3733c64f172ef8a639790cfe240c9b454311f8cbfe74f96",
  "0xc3aa105618977410007f32f4eefe0b3eab174af6dac0d95829b92e18912bfbe3",
  "0x0d3499b9350c0ac6add6e0076775de67baee79c5b691f3a4f9317dcb974db599",
  "0x1c5d4645e72d75c0152153a5fe8679a3c0a7ba6cfe3b91986e647c4b26c144bc",
  "0xd07ef0087259b42adc11497be275f42091c6ef51becccd113be860e1b48a5109",
  "0xa4895607d62c8e116357ba7d102e08b5636840e0816a608f3a1fc9d0a1077569",
  "0x094cf24d193ac65c8a450188d16282fba8ee8c5a7539b751857d231f4380c2dd",
]);

const DEFAULT_GENERATED_MATERIAL_OUT =
  "output/sccp-bsc-production/groth16-material";
const PRODUCTION_SNARKJS_R1CS_MIN_BYTES = 64 * 1024;
const PRODUCTION_SNARKJS_ZKEY_MIN_BYTES = 64 * 1024;
const PRODUCTION_FULL_SCCP_MIN_R1CS_CONSTRAINTS = 4096;
const SNARKJS_R1CS_MAGIC = Object.freeze([0x72, 0x31, 0x63, 0x73]);
const SNARKJS_ZKEY_MAGIC = Object.freeze([0x7a, 0x6b, 0x65, 0x79]);

const trim = (value) => String(value ?? "").trim();

function ownValue(record, key) {
  return record && Object.prototype.hasOwnProperty.call(record, key)
    ? record[key]
    : undefined;
}

function isRecord(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function canonicalJson(value) {
  if (value === null) return "null";
  if (typeof value === "string") return JSON.stringify(value);
  if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      throw new Error("canonical JSON number must be finite");
    }
    return JSON.stringify(value);
  }
  if (typeof value === "boolean") return value ? "true" : "false";
  if (Array.isArray(value)) {
    return `[${value.map((entry) => canonicalJson(entry)).join(",")}]`;
  }
  if (isRecord(value)) {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`)
      .join(",")}}`;
  }
  throw new Error("canonical JSON supports only JSON values");
}

function attestationSignedBody(record) {
  if (!isRecord(record)) {
    return record;
  }
  const { signature: _signature, signatures: _signatures, ...body } = record;
  return body;
}

function attestationSignaturePayload(record) {
  return Buffer.from(canonicalJson(attestationSignedBody(record)), "utf8");
}

function normalizeSignerFingerprint(value, label = "attestation signer fingerprint") {
  return normalizeHex32(value, label);
}

function parseTrustedSignerFingerprints(options = {}) {
  const raw = [
    ownValue(options, "trusted-attestation-signer"),
    ownValue(options, "trusted-attestation-signer-fingerprint"),
    ownValue(options, "trusted-attestation-signers"),
    process.env.SCCP_BSC_TRUSTED_ATTESTATION_SIGNERS,
  ]
    .filter((value) => value !== undefined && value !== null && trim(value) !== "")
    .flatMap((value) => String(value).split(/[,\s]+/u))
    .map((value) => trim(value))
    .filter(Boolean);
  return [...new Set(raw.map((value) => normalizeSignerFingerprint(value)))];
}

function publicKeyFingerprint(publicKeyPem, label) {
  const publicKey = createPublicKey(String(publicKeyPem));
  const der = publicKey.export({ format: "der", type: "spki" });
  return { publicKey, fingerprint: sha256Hex(der) };
}

function signatureBytes(value, label) {
  if (typeof value !== "string" || trim(value) === "") {
    throw new Error(`${label} is required`);
  }
  const normalized = trim(value);
  if (/^0x[0-9a-f]+$/iu.test(normalized)) {
    const hex = normalized.slice(2);
    if (hex.length % 2 !== 0) {
      throw new Error(`${label} hex must have an even number of digits`);
    }
    return Buffer.from(hex, "hex");
  }
  return Buffer.from(normalized, "base64");
}

function parseArgs(argv) {
  const args = {};
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (!token.startsWith("--")) {
      throw new Error(`Unexpected argument: ${token}`);
    }
    const key = token.slice(2);
    if (Object.prototype.hasOwnProperty.call(args, key)) {
      throw new Error(`Duplicate option: --${key}`);
    }
    const next = argv[index + 1];
    if (!next || next.startsWith("--")) {
      args[key] = "true";
    } else {
      args[key] = next;
      index += 1;
    }
  }
  return args;
}

function optionEnabled(options, key, fallback = false) {
  const value = ownValue(options, key);
  if (value === undefined || value === null || value === "") {
    return fallback;
  }
  if (value === "true" || value === true) return true;
  if (value === "false" || value === false) return false;
  throw new Error(`--${key} must be true or false.`);
}

function requiredOption(options, names, label) {
  const keys = Array.isArray(names) ? names : [names];
  for (const key of keys) {
    const value = ownValue(options, key);
    if (value !== undefined && trim(value) !== "") {
      return value;
    }
  }
  throw new Error(`${label} requires --${keys[0]}.`);
}

function optionalPath(options, names) {
  const keys = Array.isArray(names) ? names : [names];
  for (const key of keys) {
    const value = ownValue(options, key);
    if (value !== undefined && trim(value) !== "") {
      return resolve(String(value));
    }
  }
  return null;
}

function sha256Hex(bytes) {
  return `0x${createHash("sha256").update(bytes).digest("hex")}`;
}

async function fileSha256(pathName) {
  return sha256Hex(await readFile(pathName));
}

function repoRelativePath(pathName) {
  const resolved = resolve(pathName);
  const relativePath = relative(REPO_ROOT, resolved).split(/[\\/]+/u).join("/");
  return relativePath && !relativePath.startsWith("..") && !isAbsolute(relativePath)
    ? relativePath
    : resolved;
}

async function readJson(pathName, label = "JSON file") {
  try {
    return JSON.parse(await readFile(resolve(pathName), "utf8"));
  } catch (error) {
    throw new Error(`${label} could not be read as JSON: ${error.message}`);
  }
}

async function writePublicJson(pathName, value) {
  const reason = unsafeSecretReason(value);
  if (reason) {
    throw new Error(reason);
  }
  const resolved = resolve(pathName);
  await mkdir(dirname(resolved), { recursive: true });
  const temp = `${resolved}.tmp-${Date.now()}-${Math.random().toString(16).slice(2)}`;
  await writeFile(temp, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o644 });
  await rename(temp, resolved);
  return resolved;
}

async function writePublicText(pathName, value) {
  const reason = unsafeSecretReason(value);
  if (reason) {
    throw new Error(reason);
  }
  const resolved = resolve(pathName);
  await mkdir(dirname(resolved), { recursive: true });
  const temp = `${resolved}.tmp-${Date.now()}-${Math.random().toString(16).slice(2)}`;
  await writeFile(temp, value, { mode: 0o644 });
  await rename(temp, resolved);
  return resolved;
}

async function assertReadableRegularFile(pathName, label) {
  const resolved = resolve(pathName);
  const info = await lstat(resolved);
  if (info.isSymbolicLink()) {
    throw new Error(`${label} must not be a symbolic link.`);
  }
  if (!info.isFile()) {
    throw new Error(`${label} must be a regular file.`);
  }
  return resolved;
}

async function copyPublicFile(source, target, label) {
  const sourcePath = await assertReadableRegularFile(source, label);
  const targetPath = resolve(target);
  await mkdir(dirname(targetPath), { recursive: true });
  await copyFile(sourcePath, targetPath);
  return targetPath;
}

function decimalWord(value, label) {
  if (typeof value === "number" && Number.isSafeInteger(value) && value >= 0) {
    return String(value);
  }
  if (typeof value === "bigint" && value >= 0n) {
    return value.toString(10);
  }
  const text = trim(value);
  if (/^0x[0-9a-f]+$/iu.test(text)) {
    return BigInt(text).toString(10);
  }
  if (/^[0-9]+$/u.test(text)) {
    return BigInt(text).toString(10);
  }
  throw new Error(`${label} must be an unsigned integer word.`);
}

function snarkjsG1(point, label) {
  if (!Array.isArray(point) || point.length < 2) {
    throw new Error(`${label} must be a SnarkJS G1 point.`);
  }
  return [
    decimalWord(point[0], `${label}[0]`),
    decimalWord(point[1], `${label}[1]`),
  ];
}

function snarkjsG2(point, label) {
  if (
    !Array.isArray(point) ||
    point.length < 2 ||
    !Array.isArray(point[0]) ||
    !Array.isArray(point[1]) ||
    point[0].length < 2 ||
    point[1].length < 2
  ) {
    throw new Error(`${label} must be a SnarkJS G2 point.`);
  }
  return [
    decimalWord(point[0][0], `${label}[0][0]`),
    decimalWord(point[0][1], `${label}[0][1]`),
    decimalWord(point[1][0], `${label}[1][0]`),
    decimalWord(point[1][1], `${label}[1][1]`),
  ];
}

function snarkjsIc(points, label) {
  if (!Array.isArray(points) || points.length !== 10) {
    throw new Error(`${label} must contain exactly 10 G1 points.`);
  }
  return points.flatMap((point, index) => snarkjsG1(point, `${label}[${index}]`));
}

export function snarkjsVerificationKeyToBscVerifierMaterial(
  verificationKey,
  options = {},
) {
  if (
    !verificationKey ||
    typeof verificationKey !== "object" ||
    Array.isArray(verificationKey)
  ) {
    throw new Error("SnarkJS verification key must be a JSON object.");
  }
  if (verificationKey.protocol && verificationKey.protocol !== "groth16") {
    throw new Error("SnarkJS verification key protocol must be groth16.");
  }
  if (verificationKey.curve && !["bn128", "bn254"].includes(verificationKey.curve)) {
    throw new Error("SnarkJS verification key curve must be bn128/bn254.");
  }
  const nPublic = Number(verificationKey.nPublic);
  if (!Number.isSafeInteger(nPublic) || nPublic !== 9) {
    throw new Error("SnarkJS verification key nPublic must be 9.");
  }
  const profile = normalizeBscNetworkProfile(
    ownValue(options, "bscNetwork") ?? ownValue(options, "network") ?? "testnet",
  );
  const material = {
    schema: BSC_GROTH16_VERIFIER_KEY_SCHEMA,
    routeId: ROUTE_ID,
    assetKey: ASSET_KEY,
    bscNetwork: profile.key,
    chain: profile.chain,
    chainIdHex: profile.chainIdHex,
    networkId: profile.networkIdHex,
    networkIdHex: profile.networkIdHex,
    proofBackend: BSC_EVM_GROTH16_BACKEND,
    proofFamily: SCCP_PROOF_FAMILY_STARK_FRI,
    sourceDomain: SCCP_DOMAIN_SORA,
    targetDomain: SCCP_DOMAIN_BSC,
    publicInputCount: 9,
    publicSignalNames: [...BSC_GROTH16_PUBLIC_SIGNAL_NAMES],
    alpha1: snarkjsG1(verificationKey.vk_alpha_1, "vk_alpha_1"),
    beta2: snarkjsG2(verificationKey.vk_beta_2, "vk_beta_2"),
    gamma2: snarkjsG2(verificationKey.vk_gamma_2, "vk_gamma_2"),
    delta2: snarkjsG2(verificationKey.vk_delta_2, "vk_delta_2"),
    ic: snarkjsIc(verificationKey.IC, "IC"),
  };
  const verifierKeyHash = bscGroth16VerifierKeyHash(material);
  material.expectedVerifierKeyHash = verifierKeyHash;
  material.verifierKeyHash = verifierKeyHash;
  normalizeVerifierMaterial(material, profile);
  return material;
}

export function generateBscSignalBindingCircuitSource() {
  return `pragma circom 2.1.6;

template SccpBscSignalBindingV1() {
  signal input publicSignals[9];
  signal input witnessSignals[9];
  signal diff[9];

  for (var i = 0; i < 9; i++) {
    diff[i] <== witnessSignals[i] - publicSignals[i];
    diff[i] * diff[i] === 0;
  }
}

component main { public [publicSignals] } = SccpBscSignalBindingV1();
`;
}

function labelByteArgs(labelHash) {
  const hex = normalizeHex32(labelHash, "BSC Groth16 signal label").slice(2);
  return Array.from({ length: 32 }, (_, index) => `0x${hex.slice(index * 2, index * 2 + 2)}`).join(", ");
}

function fullSignalComponentLines() {
  return BSC_GROTH16_PUBLIC_SIGNAL_NAMES.map((name, index) => {
    const camelName = name.replace(/_([a-z])/gu, (_, char) => char.toUpperCase());
    const inputName = `${camelName}Bits`;
    return `  component ${camelName} = SccpBscLabeledKeccakSignal(${labelByteArgs(BSC_GROTH16_PUBLIC_SIGNAL_LABEL_HASHES[index])});
  for (var ${camelName}Index = 0; ${camelName}Index < 256; ${camelName}Index++) {
    ${camelName}.valueBits[${camelName}Index] <== ${inputName}[${camelName}Index];
  }
  ${camelName}.publicSignal <== publicSignals[${index}];`;
  }).join("\n\n");
}

export function generateBscFullMessageCircuitSource() {
  return `pragma circom 2.1.6;

// SCCP BSC full-message circuit profile:
// ${BSC_FULL_SCCP_CIRCUIT_PROFILE}
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

${fullSignalComponentLines()}
}

component main { public [publicSignals] } = SccpBscFullMessageV1();
`;
}

function runCommand(command, args, options = {}) {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(command, args, {
      cwd: options.cwd,
      env: options.env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString("utf8");
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString("utf8");
    });
    child.on("error", reject);
    child.on("close", (code) => {
      if (code === 0) {
        resolvePromise({ stdout, stderr });
        return;
      }
      const output = `${stdout}\n${stderr}`.trim().split(/\n/u).slice(-30).join("\n");
      reject(
        new Error(
          `${command} ${args.join(" ")} failed with exit ${code}${
            output ? `:\n${output}` : ""
          }`,
        ),
      );
    });
  });
}

function commandValue(options, key, fallback) {
  return trim(ownValue(options, key) ?? process.env[key.toUpperCase().replace(/-/gu, "_")] ?? fallback);
}

function defaultMaterialOut(profile) {
  return resolve(DEFAULT_GENERATED_MATERIAL_OUT, profile.key);
}

function productionBlockersForMaterial({
  circuitProfile,
  localPtau,
  localPhase2,
  attestations,
  attestationValidationBlockers = [],
}) {
  const blockers = [];
  if (circuitProfile !== BSC_FULL_SCCP_CIRCUIT_PROFILE) {
    blockers.push(
      "circuit profile is only public-signal binding; full SCCP message/finality semantics are not proven",
    );
  }
  if (localPtau) {
    blockers.push("trusted setup uses a locally generated Powers of Tau file");
  }
  if (localPhase2) {
    blockers.push("phase2 zkey contribution is local single-contributor material");
  }
  if (!attestations.semanticSccpCircuit) {
    blockers.push("missing semantic SCCP circuit attestation");
  }
  if (!attestations.circuitSecurity) {
    blockers.push("missing circuit security audit attestation");
  }
  if (!attestations.trustedSetup) {
    blockers.push("missing trusted setup ceremony attestation");
  }
  if (!attestations.reproducibleBuild) {
    blockers.push("missing reproducible build attestation");
  }
  blockers.push(...attestationValidationBlockers);
  return blockers;
}

async function attestationReference(pathName, label) {
  if (!pathName) {
    return null;
  }
  const resolved = await assertReadableRegularFile(pathName, `${label} attestation`);
  let record;
  try {
    record = await readJson(resolved, `${label} attestation`);
  } catch (error) {
    return {
      path: repoRelativePath(resolved),
      sha256: await fileSha256(resolved),
      schema: null,
      record: null,
      readError: error instanceof Error ? error.message : String(error),
    };
  }
  const secretReason = unsafeSecretReason(record, `${label} attestation`);
  if (secretReason) {
    throw new Error(secretReason);
  }
  return {
    path: repoRelativePath(resolved),
    sha256: await fileSha256(resolved),
    schema: isRecord(record) ? ownValue(record, "schema") ?? null : null,
    record,
  };
}

async function buildAttestationReferences(options) {
  return {
    semanticSccpCircuit: await attestationReference(
      optionalPath(options, ["semantic-attestation", "semantic-sccp-attestation"]),
      "semantic SCCP circuit",
    ),
    circuitSecurity: await attestationReference(
      optionalPath(options, ["circuit-security-attestation", "circuit-audit"]),
      "circuit security",
    ),
    trustedSetup: await attestationReference(
      optionalPath(options, ["trusted-setup-attestation", "ceremony-attestation"]),
      "trusted setup",
    ),
    reproducibleBuild: await attestationReference(
      optionalPath(options, ["reproducible-build-attestation"]),
      "reproducible build",
    ),
  };
}

function publicAttestationReferences(attestations) {
  return Object.fromEntries(
    Object.entries(attestations).map(([key, value]) => [
      key,
      value
        ? {
            path: value.path,
            sha256: value.sha256,
            schema: value.schema,
            ...(value.signatureSummary
              ? { signature: value.signatureSummary }
              : {}),
            ...(value.readError ? { readError: value.readError } : {}),
          }
        : null,
    ]),
  );
}

function attestationSignatureBlockers(entry, trustedSignerFingerprints, label) {
  if (!entry || entry.readError || !isRecord(entry.record)) {
    return [];
  }
  const trusted = new Set(trustedSignerFingerprints);
  const blockers = [];
  if (trusted.size === 0) {
    blockers.push(`${label} trusted attestation signer fingerprint is required`);
  }
  const signature = ownValue(entry.record, "signature");
  if (!isRecord(signature)) {
    blockers.push(`${label} signature is required`);
    entry.signatureSummary = {
      verified: false,
      algorithm: null,
      signerFingerprint: null,
      signedPayloadSha256: sha256Hex(attestationSignaturePayload(entry.record)),
    };
    return blockers;
  }
  const summary = {
    verified: false,
    algorithm: trim(ownValue(signature, "algorithm")),
    signerFingerprint: null,
    signedPayloadSha256: sha256Hex(attestationSignaturePayload(entry.record)),
  };
  entry.signatureSummary = summary;
  if (
    trim(ownValue(signature, "schema")) !==
    BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA
  ) {
    blockers.push(
      `${label} signature schema must be ${BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA}`,
    );
  }
  if (summary.algorithm !== "ed25519") {
    blockers.push(`${label} signature algorithm must be ed25519`);
  }
  try {
    const expectedPayloadHash = normalizeSignerFingerprint(
      ownValue(signature, "signedPayloadSha256") ??
        ownValue(signature, "signed_payload_sha256"),
      `${label} signature signedPayloadSha256`,
    );
    if (expectedPayloadHash !== summary.signedPayloadSha256) {
      blockers.push(`${label} signature signedPayloadSha256 must match attestation body`);
    }
  } catch (error) {
    blockers.push(error instanceof Error ? error.message : String(error));
  }
  let publicKey;
  try {
    const result = publicKeyFingerprint(
      ownValue(signature, "publicKeyPem") ?? ownValue(signature, "public_key_pem"),
      `${label} signature publicKeyPem`,
    );
    publicKey = result.publicKey;
    const declaredFingerprint = normalizeSignerFingerprint(
      ownValue(signature, "signerFingerprint") ??
        ownValue(signature, "signer_fingerprint"),
      `${label} signature signerFingerprint`,
    );
    summary.signerFingerprint = declaredFingerprint;
    if (declaredFingerprint !== result.fingerprint) {
      blockers.push(`${label} signature signerFingerprint must match public key`);
    }
    if (trusted.size > 0 && !trusted.has(declaredFingerprint)) {
      blockers.push(`${label} signature signerFingerprint is not trusted`);
    }
  } catch (error) {
    blockers.push(error instanceof Error ? error.message : String(error));
  }
  try {
    const signatureBuffer = signatureBytes(
      ownValue(signature, "signature") ?? ownValue(signature, "signatureBase64"),
      `${label} signature`,
    );
    if (
      publicKey &&
      verifyDetachedSignature(
        null,
        attestationSignaturePayload(entry.record),
        publicKey,
        signatureBuffer,
      )
    ) {
      summary.verified = true;
    } else {
      blockers.push(`${label} detached signature verification failed`);
    }
  } catch (error) {
    blockers.push(error instanceof Error ? error.message : String(error));
  }
  return blockers;
}

function attestationSignerDiversityBlockers(attestations) {
  const rows = [
    ["semantic SCCP circuit attestation", attestations.semanticSccpCircuit],
    ["circuit security attestation", attestations.circuitSecurity],
    ["trusted setup attestation", attestations.trustedSetup],
    ["reproducible build attestation", attestations.reproducibleBuild],
  ];
  const seen = new Map();
  const blockers = [];
  for (const [label, entry] of rows) {
    const fingerprint = entry?.signatureSummary?.verified
      ? entry.signatureSummary.signerFingerprint
      : null;
    if (!fingerprint) {
      continue;
    }
    const previous = seen.get(fingerprint);
    if (previous) {
      blockers.push(
        `production Groth16 attestation signers must be role-separated; ${previous} and ${label} reuse signer ${fingerprint}`,
      );
    } else {
      seen.set(fingerprint, label);
    }
  }
  return blockers;
}

async function artifactRecord(pathName) {
  return {
    path: repoRelativePath(pathName),
    sha256: await fileSha256(pathName),
  };
}

function u32le(bytes, offset) {
  return (
    (bytes[offset] |
      (bytes[offset + 1] << 8) |
      (bytes[offset + 2] << 16) |
      (bytes[offset + 3] << 24)) >>>
    0
  );
}

function u64leSafe(bytes, offset) {
  const low = u32le(bytes, offset);
  const high = u32le(bytes, offset + 4);
  const value = high * 0x100000000 + low;
  return Number.isSafeInteger(value) ? value : null;
}

function snarkjsSectionTableBlockers(bytes, label, magic) {
  const blockers = [];
  if (bytes.length < 12) {
    return [`${label} SnarkJS section table is truncated`];
  }
  const hasMagic = magic.every((byte, index) => bytes[index] === byte);
  if (!hasMagic) {
    blockers.push(`${label} must start with SnarkJS ${label} magic bytes`);
    return blockers;
  }
  const version = u32le(bytes, 4);
  if (version < 1 || version > 2) {
    blockers.push(`${label} SnarkJS version is unsupported`);
  }
  const sectionCount = u32le(bytes, 8);
  if (sectionCount < 1 || sectionCount > 128) {
    blockers.push(`${label} SnarkJS section count is invalid`);
    return blockers;
  }
  let offset = 12;
  const sectionIds = new Set();
  for (let index = 0; index < sectionCount; index += 1) {
    if (offset + 12 > bytes.length) {
      blockers.push(`${label} SnarkJS section table is truncated`);
      return blockers;
    }
    const sectionId = u32le(bytes, offset);
    const sectionSize = u64leSafe(bytes, offset + 4);
    offset += 12;
    if (sectionId === 0) {
      blockers.push(`${label} SnarkJS section id must be non-zero`);
    }
    if (sectionIds.has(sectionId)) {
      blockers.push(`${label} SnarkJS section ids must be unique`);
    }
    sectionIds.add(sectionId);
    if (sectionSize === null || sectionSize <= 0) {
      blockers.push(`${label} SnarkJS section size is invalid`);
      return blockers;
    }
    if (sectionSize > bytes.length - offset) {
      blockers.push(`${label} SnarkJS section exceeds file size`);
      return blockers;
    }
    offset += sectionSize;
  }
  if (offset !== bytes.length) {
    blockers.push(`${label} SnarkJS section table does not consume the full file`);
  }
  return blockers;
}

async function snarkjsArtifactBlockers(pathName, label, magic, minBytes) {
  const bytes = await readFile(pathName);
  const blockers = [];
  if (bytes.length < minBytes) {
    blockers.push(`${label} must be at least ${minBytes} bytes for production material`);
  }
  blockers.push(...snarkjsSectionTableBlockers(bytes, label, magic));
  return blockers;
}

function stripCircomComments(source) {
  return String(source ?? "")
    .replace(/\/\*[\s\S]*?\*\//gu, "")
    .replace(/(^|[^:])\/\/.*$/gmu, "$1");
}

function escapedRegexFragment(value) {
  return String(value).replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}

function labelBindingRegex(labelHash) {
  const byteFragments = normalizeHex32(labelHash, "BSC Groth16 signal label")
    .slice(2)
    .match(/../gu)
    .map((byte) => `0x${byte}`);
  return new RegExp(
    `SccpBscLabeledKeccakSignal\\(\\s*${byteFragments
      .map(escapedRegexFragment)
      .join("\\s*,\\s*")}\\s*\\)`,
    "u",
  );
}

function fullCircuitSourceAnalysis(source) {
  const text = String(source ?? "");
  const code = stripCircomComments(text);
  const publicSignalIndexes = new Set(
    Array.from(code.matchAll(/publicSignals\s*\[\s*([0-8])\s*\]/gu), (match) =>
      Number(match[1]),
    ),
  );
  const labelBindingCount = BSC_GROTH16_PUBLIC_SIGNAL_LABEL_HASHES.reduce(
    (count, labelHash) => count + (labelBindingRegex(labelHash).test(code) ? 1 : 0),
    0,
  );
  const checks = {
    fullMessageCircuit: /template\s+SccpBscFullMessageV1\b/u.test(code),
    signalBindingFixture: /SccpBscSignalBindingV1|witnessSignals/u.test(text),
    unresolvedPlaceholders:
      /must connect|Implementations must wire|Leaving these surfaces visible|source surface|fixture fallback/iu.test(
        text,
      ),
    keccakPublicSignalDerivation:
      /SccpBscLabeledKeccakSignal\b/u.test(code) &&
      /Keccak\s*\(\s*512\s*,\s*256\s*\)/u.test(code),
    digestReductionModuloScalarField:
      /digestBigEndianModFr/u.test(code) &&
      /publicSignal\s*===\s*digestBigEndianModFr/u.test(code),
    valueBitBooleanConstraints:
      /valueBits\s*\[\s*byte\s*\*\s*8\s*\+\s*bit\s*\]\s*\*\s*\(\s*valueBits\s*\[\s*byte\s*\*\s*8\s*\+\s*bit\s*\]\s*-\s*1\s*\)\s*===\s*0/u.test(
        code,
      ),
    publicSignalConstraintCount: publicSignalIndexes.size,
    labelBindingCount,
  };
  const blockers = [];
  if (!checks.fullMessageCircuit) {
    blockers.push("full circuit source must define SccpBscFullMessageV1");
  }
  if (checks.signalBindingFixture) {
    blockers.push(
      "full production material must not use the signal-binding fixture circuit",
    );
  }
  if (checks.unresolvedPlaceholders) {
    blockers.push("full circuit source must not contain unresolved scaffold placeholders");
  }
  if (!checks.keccakPublicSignalDerivation) {
    blockers.push("full circuit source must derive public signals with Keccak(512, 256)");
  }
  if (!checks.digestReductionModuloScalarField) {
    blockers.push(
      "full circuit source must constrain public signals to the Keccak digest modulo the BN254 scalar field",
    );
  }
  if (!checks.valueBitBooleanConstraints) {
    blockers.push("full circuit source must boolean-constrain SCCP statement value bits");
  }
  if (checks.publicSignalConstraintCount !== 9) {
    blockers.push("full circuit source must constrain all 9 publicSignals entries");
  }
  if (checks.labelBindingCount !== 9) {
    blockers.push("full circuit source must bind all 9 Solidity signal labels in circuit code");
  }
  return { checks, blockers };
}

async function fullCircuitSourceCheck(circuitSourcePath) {
  if (!circuitSourcePath) {
    return {
      checks: {
        fullMessageCircuit: false,
        signalBindingFixture: false,
        unresolvedPlaceholders: false,
        keccakPublicSignalDerivation: false,
        digestReductionModuloScalarField: false,
        valueBitBooleanConstraints: false,
        publicSignalConstraintCount: 0,
        labelBindingCount: 0,
      },
      blockers: ["full production material requires circuit source artifact"],
    };
  }
  const source = await readFile(circuitSourcePath, "utf8");
  const analysis = fullCircuitSourceAnalysis(source);
  const requiredFragments = [
    "circomlib/circuits/gates.circom",
    "circomlib/circuits/sha256/xor3.circom",
    "circomlib/circuits/sha256/shift.circom",
    "circomlib/circuits/bitify.circom",
    "@electron-labs/keccak-circom/circuits/keccak.circom",
    "message_id",
    "payload_hash",
    "target_domain",
    "commitment_root",
    "finality_height",
    "finality_block_hash",
    "source_domain",
    "statement_hash",
    "destination_binding_hash",
    "0x091b1715f31adbc0239378caf77a4370e8348599048ec45efb203368dbcc5073",
    "0xd40cf4310af21ab1b3f12db20df99ab8fe63dbe55fc473e5456691c39c1859ac",
    "0x5f7c135fa34a3f53c3733c64f172ef8a639790cfe240c9b454311f8cbfe74f96",
    "0xc3aa105618977410007f32f4eefe0b3eab174af6dac0d95829b92e18912bfbe3",
    "0x0d3499b9350c0ac6add6e0076775de67baee79c5b691f3a4f9317dcb974db599",
    "0x1c5d4645e72d75c0152153a5fe8679a3c0a7ba6cfe3b91986e647c4b26c144bc",
    "0xd07ef0087259b42adc11497be275f42091c6ef51becccd113be860e1b48a5109",
    "0xa4895607d62c8e116357ba7d102e08b5636840e0816a608f3a1fc9d0a1077569",
    "0x094cf24d193ac65c8a450188d16282fba8ee8c5a7539b751857d231f4380c2dd",
  ];
  const blockers = [...analysis.blockers];
  for (const fragment of requiredFragments) {
    if (!source.includes(fragment)) {
      blockers.push(`full circuit source must contain ${fragment}`);
    }
  }
  return { checks: analysis.checks, blockers };
}

function parseSnarkjsR1csInfo(stdout) {
  const normalized = String(stdout ?? "");
  const constraintsMatch =
    normalized.match(/#\s*of\s*constraints\s*:\s*([0-9]+)/iu) ??
    normalized.match(/\bconstraints\s*[:=]\s*([0-9]+)/iu);
  const publicInputsMatch =
    normalized.match(/#\s*of\s*public\s*inputs\s*:\s*([0-9]+)/iu) ??
    normalized.match(/\bpublic\s*inputs\s*[:=]\s*([0-9]+)/iu);
  return {
    constraintCount: constraintsMatch ? Number(constraintsMatch[1]) : null,
    publicInputCount: publicInputsMatch ? Number(publicInputsMatch[1]) : null,
  };
}

async function snarkjsMaterialSelfCheck({
  snarkjsBin,
  r1csPath,
  zkeyPath,
  profile,
  verifierKeyHash,
  circuitProfile,
}) {
  const checks = {
    snarkjsBinary: snarkjsBin,
    r1csInfo: false,
    r1csConstraintCount: null,
    r1csPublicInputCount: null,
    zkeyVerificationKeyExport: false,
    verifierKeyHashMatches: false,
    exportedVerifierKeyHash: null,
  };
  const blockers = [];
  try {
    const info = await runCommand(snarkjsBin, ["r1cs", "info", r1csPath]);
    const parsed = parseSnarkjsR1csInfo(`${info.stdout}\n${info.stderr}`);
    checks.r1csConstraintCount = parsed.constraintCount;
    checks.r1csPublicInputCount = parsed.publicInputCount;
    checks.r1csInfo = true;
    if (parsed.publicInputCount !== 9) {
      blockers.push("SnarkJS R1CS self-check public input count must be 9.");
    }
    if (circuitProfile === BSC_FULL_SCCP_CIRCUIT_PROFILE) {
      if (
        !Number.isSafeInteger(parsed.constraintCount) ||
        parsed.constraintCount < PRODUCTION_FULL_SCCP_MIN_R1CS_CONSTRAINTS
      ) {
        blockers.push(
          `SnarkJS R1CS self-check constraint count must be at least ${PRODUCTION_FULL_SCCP_MIN_R1CS_CONSTRAINTS} for full SCCP material.`,
        );
      }
    }
  } catch (error) {
    blockers.push(
      `SnarkJS R1CS self-check failed: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  }

  let scratchDir = null;
  try {
    scratchDir = await mkdtemp(join(tmpdir(), "iroha-bsc-snarkjs-self-check-"));
    const exportedVerificationKey = join(scratchDir, "verification_key.json");
    await runCommand(snarkjsBin, [
      "zkey",
      "export",
      "verificationkey",
      zkeyPath,
      exportedVerificationKey,
    ]);
    checks.zkeyVerificationKeyExport = true;
    const exportedMaterial = snarkjsVerificationKeyToBscVerifierMaterial(
      await readJson(exportedVerificationKey, "SnarkJS exported verification key"),
      { bscNetwork: profile.key },
    );
    checks.exportedVerifierKeyHash = exportedMaterial.verifierKeyHash;
    checks.verifierKeyHashMatches =
      exportedMaterial.verifierKeyHash === verifierKeyHash;
    if (!checks.verifierKeyHashMatches) {
      blockers.push(
        "SnarkJS zkey export hash mismatch against provided verifier material.",
      );
    }
  } catch (error) {
    blockers.push(
      `SnarkJS zkey verification key self-check failed: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  } finally {
    if (scratchDir) {
      await rm(scratchDir, { recursive: true, force: true });
    }
  }
  return { checks, blockers };
}

function attestationValue(record, keys) {
  for (const key of Array.isArray(keys) ? keys : [keys]) {
    const value = ownValue(record, key);
    if (value !== undefined) {
      return value;
    }
  }
  return undefined;
}

function stringEqualsBlocker(record, keys, expected, label) {
  const value = attestationValue(record, keys);
  if (value === undefined || value === null || trim(value) === "") {
    return `${label} is required`;
  }
  return String(value) === expected ? "" : `${label} must be ${expected}`;
}

function integerEqualsBlocker(record, keys, expected, label) {
  const value = Number(attestationValue(record, keys));
  if (!Number.isSafeInteger(value)) {
    return `${label} is required`;
  }
  return value === expected ? "" : `${label} must be ${expected}`;
}

function booleanTrueBlocker(record, keys, label) {
  return attestationValue(record, keys) === true ? "" : `${label} must be true`;
}

function booleanFalseBlocker(record, keys, label) {
  return attestationValue(record, keys) === false ? "" : `${label} must be false`;
}

function integerAtLeastBlocker(record, keys, minimum, label) {
  const value = Number(attestationValue(record, keys));
  if (!Number.isSafeInteger(value)) {
    return `${label} is required`;
  }
  return value >= minimum ? "" : `${label} must be at least ${minimum}`;
}

function integerZeroBlocker(record, keys, label) {
  const value = Number(attestationValue(record, keys));
  if (!Number.isSafeInteger(value)) {
    return `${label} is required`;
  }
  return value === 0 ? "" : `${label} must be 0`;
}

function hashEqualsBlocker(record, keys, expected, label) {
  const value = attestationValue(record, keys);
  if (value === undefined || value === null || trim(value) === "") {
    return `${label} is required`;
  }
  let normalized;
  try {
    normalized = normalizeHex32(value, label);
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
  return normalized === expected ? "" : `${label} must match ${expected}`;
}

function hashPresentBlocker(record, keys, label) {
  const value = attestationValue(record, keys);
  if (value === undefined || value === null || trim(value) === "") {
    return `${label} is required`;
  }
  try {
    normalizeHex32(value, label);
    return "";
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
}

function publicSignalsBlocker(record, context, label) {
  const names = attestationValue(record, ["publicSignalNames", "public_signal_names"]);
  if (!Array.isArray(names)) {
    return `${label} publicSignalNames is required`;
  }
  return JSON.stringify(names) === JSON.stringify(context.publicSignalNames)
    ? ""
    : `${label} publicSignalNames must match BSC Groth16 public signals`;
}

function commonAttestationBlockers(entry, expectedSchema, context, label) {
  if (!entry) {
    return [];
  }
  if (entry.readError) {
    return [`${label} attestation is not valid JSON: ${entry.readError}`];
  }
  const record = entry.record;
  if (!isRecord(record)) {
    return [`${label} attestation must be a JSON object`];
  }
  const blockers = [
    stringEqualsBlocker(record, "schema", expectedSchema, `${label} schema`),
    stringEqualsBlocker(record, ["routeId", "route_id"], ROUTE_ID, `${label} routeId`),
    stringEqualsBlocker(record, ["assetKey", "asset_key"], ASSET_KEY, `${label} assetKey`),
    stringEqualsBlocker(
      record,
      ["bscNetwork", "bsc_network", "network"],
      context.profile.key,
      `${label} bscNetwork`,
    ),
    stringEqualsBlocker(record, "chain", context.profile.chain, `${label} chain`),
    stringEqualsBlocker(
      record,
      ["chainIdHex", "chain_id_hex"],
      context.profile.chainIdHex,
      `${label} chainIdHex`,
    ),
    hashEqualsBlocker(
      record,
      ["networkIdHex", "network_id_hex"],
      context.profile.networkIdHex,
      `${label} networkIdHex`,
    ),
    stringEqualsBlocker(
      record,
      ["circuitProfile", "circuit_profile"],
      context.circuitProfile,
      `${label} circuitProfile`,
    ),
    integerEqualsBlocker(
      record,
      ["publicInputCount", "public_input_count"],
      9,
      `${label} publicInputCount`,
    ),
    publicSignalsBlocker(record, context, label),
    hashEqualsBlocker(
      record,
      ["verifierKeyHash", "verifier_key_hash"],
      context.verifierKeyHash,
      `${label} verifierKeyHash`,
    ),
    hashEqualsBlocker(
      record,
      ["r1csSha256", "r1cs_sha256", "proofArtifactHash", "proof_artifact_hash"],
      context.artifacts.r1cs.sha256,
      `${label} r1csSha256`,
    ),
    hashEqualsBlocker(
      record,
      ["provingKeySha256", "proving_key_sha256", "provingKeyHash", "proving_key_hash"],
      context.artifacts.provingKey.sha256,
      `${label} provingKeySha256`,
    ),
    hashEqualsBlocker(
      record,
      ["snarkjsVerificationKeySha256", "snarkjs_verification_key_sha256"],
      context.artifacts.snarkjsVerificationKey.sha256,
      `${label} snarkjsVerificationKeySha256`,
    ),
    hashEqualsBlocker(
      record,
      ["bscVerifierKeySha256", "bsc_verifier_key_sha256"],
      context.artifacts.bscVerifierKey.sha256,
      `${label} bscVerifierKeySha256`,
    ),
  ];
  if (context.artifacts.circuitSource) {
    blockers.push(
      hashEqualsBlocker(
        record,
        ["circuitSourceSha256", "circuit_source_sha256"],
        context.artifacts.circuitSource.sha256,
        `${label} circuitSourceSha256`,
      ),
    );
  }
  return blockers.filter(Boolean);
}

function validateSemanticAttestation(entry, context) {
  const label = "semantic SCCP circuit";
  const record = entry?.record;
  return [
    ...commonAttestationBlockers(
      entry,
      BSC_GROTH16_SEMANTIC_ATTESTATION_SCHEMA,
      context,
      label,
    ),
    ...(isRecord(record)
      ? [
          booleanTrueBlocker(
            record,
            ["fullSccpMessageSemantics", "full_sccp_message_semantics"],
            `${label} fullSccpMessageSemantics`,
          ),
          booleanTrueBlocker(
            record,
            ["sourceFinalitySemantics", "source_finality_semantics"],
            `${label} sourceFinalitySemantics`,
          ),
          booleanTrueBlocker(
            record,
            ["destinationBindingSemantics", "destination_binding_semantics"],
            `${label} destinationBindingSemantics`,
          ),
          booleanTrueBlocker(
            record,
            ["publicSignalDerivationSemantics", "public_signal_derivation_semantics"],
            `${label} publicSignalDerivationSemantics`,
          ),
          booleanTrueBlocker(
            record,
            ["negativeCaseCoverage", "negative_case_coverage"],
            `${label} negativeCaseCoverage`,
          ),
        ].filter(Boolean)
      : []),
  ];
}

function validateCircuitSecurityAttestation(entry, context) {
  const label = "circuit security";
  const record = entry?.record;
  return [
    ...commonAttestationBlockers(
      entry,
      BSC_GROTH16_CIRCUIT_SECURITY_ATTESTATION_SCHEMA,
      context,
      label,
    ),
    ...(isRecord(record)
      ? [
          stringEqualsBlocker(record, ["auditResult", "audit_result"], "pass", `${label} auditResult`),
          booleanTrueBlocker(record, ["approved", "productionApproved"], `${label} approved`),
          integerZeroBlocker(record, ["criticalFindings", "critical_findings"], `${label} criticalFindings`),
          integerZeroBlocker(record, ["highFindings", "high_findings"], `${label} highFindings`),
          integerZeroBlocker(record, ["unresolvedFindings", "unresolved_findings"], `${label} unresolvedFindings`),
        ].filter(Boolean)
      : []),
  ];
}

function validateTrustedSetupAttestation(entry, context) {
  const label = "trusted setup";
  const record = entry?.record;
  return [
    ...commonAttestationBlockers(
      entry,
      BSC_GROTH16_TRUSTED_SETUP_ATTESTATION_SCHEMA,
      context,
      label,
    ),
    ...(isRecord(record)
      ? [
          stringEqualsBlocker(record, ["ceremonyResult", "ceremony_result"], "pass", `${label} ceremonyResult`),
          booleanFalseBlocker(
            record,
            ["localSingleContributor", "local_single_contributor"],
            `${label} localSingleContributor`,
          ),
          integerAtLeastBlocker(
            record,
            ["minimumContributors", "minimum_contributors"],
            2,
            `${label} minimumContributors`,
          ),
          booleanTrueBlocker(
            record,
            ["toxicWasteDestroyed", "toxic_waste_destroyed"],
            `${label} toxicWasteDestroyed`,
          ),
          hashPresentBlocker(
            record,
            ["contributionTranscriptSha256", "contribution_transcript_sha256"],
            `${label} contributionTranscriptSha256`,
          ),
        ].filter(Boolean)
      : []),
  ];
}

function validateReproducibleBuildAttestation(entry, context) {
  const label = "reproducible build";
  const record = entry?.record;
  return [
    ...commonAttestationBlockers(
      entry,
      BSC_GROTH16_REPRODUCIBLE_BUILD_ATTESTATION_SCHEMA,
      context,
      label,
    ),
    ...(isRecord(record)
      ? [
          booleanTrueBlocker(record, ["reproducible", "reproducibleBuild"], `${label} reproducible`),
          integerAtLeastBlocker(
            record,
            ["independentRebuilders", "independent_rebuilders"],
            2,
            `${label} independentRebuilders`,
          ),
          hashPresentBlocker(
            record,
            ["buildTranscriptSha256", "build_transcript_sha256"],
            `${label} buildTranscriptSha256`,
          ),
        ].filter(Boolean)
      : []),
  ];
}

function validateAttestationsForMaterial(
  attestations,
  context,
  trustedSignerFingerprints = [],
) {
  const blockers = [
    ...validateSemanticAttestation(attestations.semanticSccpCircuit, context),
    ...attestationSignatureBlockers(
      attestations.semanticSccpCircuit,
      trustedSignerFingerprints,
      "semantic SCCP circuit attestation",
    ),
    ...validateCircuitSecurityAttestation(attestations.circuitSecurity, context),
    ...attestationSignatureBlockers(
      attestations.circuitSecurity,
      trustedSignerFingerprints,
      "circuit security attestation",
    ),
    ...validateTrustedSetupAttestation(attestations.trustedSetup, context),
    ...attestationSignatureBlockers(
      attestations.trustedSetup,
      trustedSignerFingerprints,
      "trusted setup attestation",
    ),
    ...validateReproducibleBuildAttestation(attestations.reproducibleBuild, context),
    ...attestationSignatureBlockers(
      attestations.reproducibleBuild,
      trustedSignerFingerprints,
      "reproducible build attestation",
    ),
  ];
  blockers.push(...attestationSignerDiversityBlockers(attestations));
  return blockers;
}

async function createLocalPtau({ snarkjsBin, outDir, power }) {
  if (!Number.isSafeInteger(power) || power < 5 || power > 28) {
    throw new Error("--create-local-ptau-power must be an integer from 5 to 28.");
  }
  const initial = join(outDir, `powersOfTau28_hez_${power.toString().padStart(2, "0")}_0000.ptau`);
  const contributed = join(outDir, `powersOfTau28_hez_${power.toString().padStart(2, "0")}_0001.ptau`);
  const finalPtau = join(outDir, `powersOfTau28_hez_${power.toString().padStart(2, "0")}_final.ptau`);
  await runCommand(snarkjsBin, [
    "powersoftau",
    "new",
    "bn128",
    String(power),
    initial,
  ]);
  await runCommand(snarkjsBin, [
    "powersoftau",
    "contribute",
    initial,
    contributed,
    "--name=local testnet candidate",
    "-v",
    `-e=${randomBytes(32).toString("hex")}`,
  ]);
  await runCommand(snarkjsBin, [
    "powersoftau",
    "prepare",
    "phase2",
    contributed,
    finalPtau,
  ]);
  return finalPtau;
}

async function runSnarkjsSetup({
  snarkjsBin,
  r1csPath,
  ptauPath,
  outDir,
  artifactStem = BSC_SIGNAL_BINDING_CIRCUIT_PROFILE,
}) {
  const zkeyInitial = join(outDir, `${artifactStem}.0000.zkey`);
  const zkeyFinal = join(outDir, `${artifactStem}.final.zkey`);
  const snarkjsVerifierKey = join(outDir, `${artifactStem}.snarkjs-verification-key.json`);
  await runCommand(snarkjsBin, ["groth16", "setup", r1csPath, ptauPath, zkeyInitial]);
  await runCommand(snarkjsBin, [
    "zkey",
    "contribute",
    zkeyInitial,
    zkeyFinal,
    "--name=local candidate phase2",
    "-v",
    `-e=${randomBytes(32).toString("hex")}`,
  ]);
  await runCommand(snarkjsBin, [
    "zkey",
    "export",
    "verificationkey",
    zkeyFinal,
    snarkjsVerifierKey,
  ]);
  return { zkeyFinal, snarkjsVerifierKey };
}

async function generateMaterialFromVerificationKey({
  profile,
  outDir,
  snarkjsVerifierKeyPath,
  r1csPath,
  zkeyPath,
  circuitSourcePath,
  wasmPath = null,
  symPath = null,
  ptauPath = null,
  localPtau = false,
  localPhase2 = false,
  circuitProfile = BSC_SIGNAL_BINDING_CIRCUIT_PROFILE,
  attestations = {},
  trustedSignerFingerprints = [],
  snarkjsBin = "snarkjs",
}) {
  const verifierMaterial = snarkjsVerificationKeyToBscVerifierMaterial(
    await readJson(snarkjsVerifierKeyPath, "SnarkJS verification key"),
    { bscNetwork: profile.key },
  );
  const bscVerifierKeyPath = join(outDir, `${profile.key}-bsc-groth16-verifier-key.json`);
  await writePublicJson(bscVerifierKeyPath, verifierMaterial);
  const artifacts = {
    ...(circuitSourcePath
      ? { circuitSource: await artifactRecord(circuitSourcePath) }
      : {}),
    r1cs: await artifactRecord(r1csPath),
    provingKey: await artifactRecord(zkeyPath),
    snarkjsVerificationKey: await artifactRecord(snarkjsVerifierKeyPath),
    bscVerifierKey: await artifactRecord(bscVerifierKeyPath),
    ...(wasmPath ? { witnessWasm: await artifactRecord(wasmPath) } : {}),
    ...(symPath ? { symbols: await artifactRecord(symPath) } : {}),
    ...(ptauPath ? { powersOfTau: await artifactRecord(ptauPath) } : {}),
  };
  const context = {
    profile,
    circuitProfile,
    publicSignalNames: [...BSC_GROTH16_PUBLIC_SIGNAL_NAMES],
    verifierKeyHash: verifierMaterial.verifierKeyHash,
    artifacts,
  };
  const sourceCheck =
    circuitProfile === BSC_FULL_SCCP_CIRCUIT_PROFILE
      ? await fullCircuitSourceCheck(circuitSourcePath)
      : null;
  const artifactBlockers = [
    ...(sourceCheck ? sourceCheck.blockers : []),
    ...(await snarkjsArtifactBlockers(
      r1csPath,
      "R1CS",
      SNARKJS_R1CS_MAGIC,
      PRODUCTION_SNARKJS_R1CS_MIN_BYTES,
    )),
    ...(await snarkjsArtifactBlockers(
      zkeyPath,
      "zkey",
      SNARKJS_ZKEY_MAGIC,
      PRODUCTION_SNARKJS_ZKEY_MIN_BYTES,
    )),
  ];
  const selfCheck = await snarkjsMaterialSelfCheck({
    snarkjsBin,
    r1csPath,
    zkeyPath,
    profile,
    verifierKeyHash: verifierMaterial.verifierKeyHash,
    circuitProfile,
  });
  const attestationValidationBlockers = validateAttestationsForMaterial(
    attestations,
    context,
    trustedSignerFingerprints,
  );
  const productionBlockers = productionBlockersForMaterial({
    circuitProfile,
    localPtau,
    localPhase2,
    attestations,
    attestationValidationBlockers,
  }).concat(artifactBlockers, selfCheck.blockers);
  const manifest = {
    schema: BSC_GROTH16_MATERIAL_MANIFEST_SCHEMA,
    generatedAt: new Date().toISOString(),
    routeId: ROUTE_ID,
    assetKey: ASSET_KEY,
    bscNetwork: profile.key,
    chain: profile.chain,
    chainIdHex: profile.chainIdHex,
    networkIdHex: profile.networkIdHex,
    proofBackend: BSC_EVM_GROTH16_BACKEND,
    proofFamily: SCCP_PROOF_FAMILY_STARK_FRI,
    sourceDomain: SCCP_DOMAIN_SORA,
    targetDomain: SCCP_DOMAIN_BSC,
    circuitProfile,
    publicInputCount: 9,
    publicSignalNames: [...BSC_GROTH16_PUBLIC_SIGNAL_NAMES],
    verifierKeyHash: verifierMaterial.verifierKeyHash,
    productionReady: productionBlockers.length === 0,
    productionBlockers,
    artifacts,
    trustedSetup: {
      localPowersOfTau: Boolean(localPtau),
      localPhase2Contribution: Boolean(localPhase2),
      contributionMaterialPersisted: false,
    },
    selfChecks: {
      snarkjs: selfCheck.checks,
      ...(sourceCheck ? { circuitSource: sourceCheck.checks } : {}),
    },
    attestationTrustPolicy: {
      signatureSchema: BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA,
      requiredAlgorithm: "ed25519",
      trustedSignerFingerprints: [...trustedSignerFingerprints],
    },
    attestations: publicAttestationReferences(attestations),
    nextStep:
      productionBlockers.length === 0
        ? "Use the verifier key for deployment, then bind r1cs/zkey/verifier hashes through native-prover-bundle and route-manifest."
        : "Do not deploy this as production. Replace or attest the circuit/setup material, then rebuild the native-prover bundle and route manifest.",
  };
  const manifestPath = join(outDir, `${profile.key}-bsc-groth16-material.manifest.json`);
  await writePublicJson(manifestPath, manifest);
  return {
    ok: true,
    productionReady: manifest.productionReady,
    productionBlockers: manifest.productionBlockers,
    outDir,
    manifest: manifestPath,
    verifierKey: bscVerifierKeyPath,
    verifierKeyHash: verifierMaterial.verifierKeyHash,
    proofArtifact: r1csPath,
    provingKey: zkeyPath,
  };
}

export async function generateBscGroth16Material(options = {}) {
  const profile = normalizeBscNetworkProfile(
    ownValue(options, "bsc-network") ?? ownValue(options, "network") ?? "testnet",
  );
  const outDir = resolve(ownValue(options, "out-dir") ?? defaultMaterialOut(profile));
  const circuitProfile = trim(
    ownValue(options, "circuit-profile") ?? BSC_SIGNAL_BINDING_CIRCUIT_PROFILE,
  );
  if (
    circuitProfile !== BSC_SIGNAL_BINDING_CIRCUIT_PROFILE &&
    circuitProfile !== BSC_FULL_SCCP_CIRCUIT_PROFILE
  ) {
    throw new Error(
      `--circuit-profile must be ${BSC_SIGNAL_BINDING_CIRCUIT_PROFILE} or ${BSC_FULL_SCCP_CIRCUIT_PROFILE}.`,
    );
  }
  const externalCircuitSource = optionalPath(options, "circuit-source");
  if (circuitProfile === BSC_FULL_SCCP_CIRCUIT_PROFILE && !externalCircuitSource) {
    throw new Error(
      `--circuit-profile ${BSC_FULL_SCCP_CIRCUIT_PROFILE} requires --circuit-source with an audited full-message Circom source.`,
    );
  }
  const circomBin = commandValue(options, "circom-bin", "circom2");
  const snarkjsBin = commandValue(options, "snarkjs-bin", "snarkjs");
  const createLocalPowerValue = ownValue(options, "create-local-ptau-power");
  const createLocalPower =
    createLocalPowerValue === undefined ? null : Number(createLocalPowerValue);
  const localPtauRequested = createLocalPower !== null;
  if (localPtauRequested && !optionEnabled(options, "allow-local-testnet-setup")) {
    throw new Error(
      "--create-local-ptau-power requires --allow-local-testnet-setup true.",
    );
  }
  if (localPtauRequested && profile.key !== "testnet") {
    throw new Error("local Powers of Tau generation is only allowed for testnet candidates.");
  }
  await mkdir(outDir, { recursive: true });
  const trustedSignerFingerprints = parseTrustedSignerFingerprints(options);
  const artifactStem = circuitProfile;
  const circuitSourcePath = join(outDir, `${artifactStem}.circom`);
  if (externalCircuitSource) {
    await copyPublicFile(
      externalCircuitSource,
      circuitSourcePath,
      "circuit source",
    );
  } else {
    await writePublicText(circuitSourcePath, generateBscSignalBindingCircuitSource());
  }
  await runCommand(circomBin, [
    circuitSourcePath,
    "--r1cs",
    "--wasm",
    "--sym",
    "-o",
    outDir,
  ]);
  const r1csPath = join(outDir, `${artifactStem}.r1cs`);
  const wasmPath = join(outDir, `${artifactStem}_js`, `${artifactStem}.wasm`);
  const symPath = join(outDir, `${artifactStem}.sym`);
  const ptauPath = localPtauRequested
    ? await createLocalPtau({ snarkjsBin, outDir, power: createLocalPower })
    : await assertReadableRegularFile(requiredOption(options, "ptau", "Powers of Tau file"), "Powers of Tau file");
  const { zkeyFinal, snarkjsVerifierKey } = await runSnarkjsSetup({
    snarkjsBin,
    r1csPath,
    ptauPath,
    outDir,
    artifactStem,
  });
  const attestations = await buildAttestationReferences(options);
  return generateMaterialFromVerificationKey({
    profile,
    outDir,
    snarkjsVerifierKeyPath: snarkjsVerifierKey,
    r1csPath,
    zkeyPath: zkeyFinal,
    circuitSourcePath,
    wasmPath,
    symPath,
    ptauPath,
    localPtau: localPtauRequested,
    localPhase2: true,
    circuitProfile,
    attestations,
    trustedSignerFingerprints,
    snarkjsBin,
  });
}

export async function materializeBscGroth16Material(options = {}) {
  const profile = normalizeBscNetworkProfile(
    ownValue(options, "bsc-network") ?? ownValue(options, "network") ?? "testnet",
  );
  const outDir = resolve(
    ownValue(options, "out-dir") ??
      join(DEFAULT_NATIVE_EVM_PROVER_ARTIFACT_ROOT, profile.key),
  );
  await mkdir(outDir, { recursive: true });
  const r1csPath = await copyPublicFile(
    requiredOption(options, "r1cs", "R1CS artifact"),
    join(outDir, basename(requiredOption(options, "r1cs", "R1CS artifact"))),
    "R1CS artifact",
  );
  const zkeyPath = await copyPublicFile(
    requiredOption(options, "zkey", "proving key"),
    join(outDir, basename(requiredOption(options, "zkey", "proving key"))),
    "proving key",
  );
  const snarkjsVerifierKeyPath = await copyPublicFile(
    requiredOption(
      options,
      ["snarkjs-verifier-key", "verification-key"],
      "SnarkJS verification key",
    ),
    join(
      outDir,
      basename(
        requiredOption(
          options,
          ["snarkjs-verifier-key", "verification-key"],
          "SnarkJS verification key",
        ),
      ),
    ),
    "SnarkJS verification key",
  );
  const circuitSourceInput = optionalPath(options, "circuit-source");
  const circuitSourcePath = circuitSourceInput
    ? await copyPublicFile(
        circuitSourceInput,
        join(outDir, basename(circuitSourceInput)),
        "circuit source",
      )
    : null;
  const attestations = await buildAttestationReferences(options);
  const trustedSignerFingerprints = parseTrustedSignerFingerprints(options);
  const circuitProfile = trim(
    ownValue(options, "circuit-profile") ?? BSC_FULL_SCCP_CIRCUIT_PROFILE,
  );
  const snarkjsBin = commandValue(options, "snarkjs-bin", "snarkjs");
  return generateMaterialFromVerificationKey({
    profile,
    outDir,
    snarkjsVerifierKeyPath,
    r1csPath,
    zkeyPath,
    circuitSourcePath,
    localPtau: false,
    localPhase2: false,
    circuitProfile,
    attestations,
    trustedSignerFingerprints,
    snarkjsBin,
  });
}

function usage() {
  return `Usage:
  node scripts/sccp_bsc_groth16_material.mjs generate --bsc-network testnet --ptau <phase2.ptau> [--circuit-profile ${BSC_SIGNAL_BINDING_CIRCUIT_PROFILE}|${BSC_FULL_SCCP_CIRCUIT_PROFILE}] [--circuit-source <full-message.circom>] [--out-dir ${DEFAULT_GENERATED_MATERIAL_OUT}/testnet] [--circom-bin circom2] [--snarkjs-bin snarkjs]
  node scripts/sccp_bsc_groth16_material.mjs generate --bsc-network testnet --create-local-ptau-power 8 --allow-local-testnet-setup true [--out-dir ${DEFAULT_GENERATED_MATERIAL_OUT}/testnet]
  node scripts/sccp_bsc_groth16_material.mjs materialize --bsc-network testnet|mainnet --r1cs <file.r1cs> --zkey <file.zkey> --snarkjs-verifier-key <verification_key.json> --circuit-source <full-message.circom> --semantic-attestation <json> --circuit-security-attestation <json> --trusted-setup-attestation <json> --reproducible-build-attestation <json> --trusted-attestation-signer <0x...> [--out-dir ${DEFAULT_NATIVE_EVM_PROVER_ARTIFACT_ROOT}/testnet]

The generate command creates real Circom/SnarkJS Groth16 candidate material for
the BSC verifier's 9 public signal words using circuit profile
${BSC_SIGNAL_BINDING_CIRCUIT_PROFILE} by default. Full-message generation
requires --circuit-profile ${BSC_FULL_SCCP_CIRCUIT_PROFILE} plus an audited
--circuit-source. Materialize records circuit-source self-checks and fails
closed unless the source constrains all 9 labeled Keccak public signals. Generated
local setup material is not production-ready unless the full SCCP circuit
semantics and ceremony/build evidence are supplied through materialize. Production
attestations must carry detached Ed25519 signatures from a configured trusted
signer fingerprint.`;
}

export async function main(argv = process.argv.slice(2)) {
  const [command, ...rest] = argv;
  if (!command || command === "--help" || command === "-h" || command === "help") {
    return { help: usage() };
  }
  const options = parseArgs(rest);
  switch (command) {
    case "generate":
      return generateBscGroth16Material(options);
    case "materialize":
      return materializeBscGroth16Material(options);
    default:
      throw new Error(`Unknown command: ${command}\n${usage()}`);
  }
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  main()
    .then((result) => {
      if (result?.help) {
        console.log(result.help);
      } else {
        console.log(JSON.stringify(result, null, 2));
      }
    })
    .catch((error) => {
      console.error(error instanceof Error ? error.message : String(error));
      process.exitCode = 1;
    });
}
