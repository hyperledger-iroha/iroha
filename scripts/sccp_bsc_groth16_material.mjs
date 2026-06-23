#!/usr/bin/env node
// Purpose: generate and inspect public BSC SCCP Groth16 circuit/proving
// material without writing operator credentials. Locally generated setup
// output is marked as a production candidate only; production-ready status
// requires externally audited circuit semantics and ceremony evidence.
import { spawn } from "node:child_process";
import {
  createHash,
  createPrivateKey,
  createPublicKey,
  randomBytes,
  sign as signDetachedPayload,
  verify as verifyDetachedSignature,
} from "node:crypto";
import { createReadStream, existsSync } from "node:fs";
import {
  copyFile,
  lstat,
  mkdtemp,
  mkdir,
  open,
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
  win32,
} from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { keccak_256 } from "../javascript/iroha_js/node_modules/@noble/hashes/sha3.js";
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
export const BSC_GROTH16_ATTESTATION_REQUEST_PACKAGE_SCHEMA =
  "iroha-sccp-bsc-groth16-attestation-request-package/v1";
export const BSC_GROTH16_PROOF_SELF_TEST_SCHEMA =
  "iroha-sccp-bsc-groth16-proof-self-test/v1";
export const BSC_SIGNAL_BINDING_CIRCUIT_PROFILE =
  "sccp-bsc-signal-binding-v1";
export const BSC_FULL_SCCP_CIRCUIT_PROFILE = "sccp-bsc-full-message-v1";
export const DEFAULT_BSC_FULL_MESSAGE_CIRCUIT_SOURCE =
  "artifacts/sccp-bsc/circuits/sccp-bsc-full-message-v1.circom";
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
const BSC_GROTH16_ATTESTATION_ROLE_SPECS = Object.freeze([
  Object.freeze({
    key: "semanticSccpCircuit",
    label: "semantic SCCP circuit",
    optionNames: Object.freeze(["semantic-attestation", "semantic-sccp-attestation"]),
    expectedSchema: BSC_GROTH16_SEMANTIC_ATTESTATION_SCHEMA,
  }),
  Object.freeze({
    key: "circuitSecurity",
    label: "circuit security",
    optionNames: Object.freeze(["circuit-security-attestation", "circuit-audit"]),
    expectedSchema: BSC_GROTH16_CIRCUIT_SECURITY_ATTESTATION_SCHEMA,
  }),
  Object.freeze({
    key: "trustedSetup",
    label: "trusted setup",
    optionNames: Object.freeze(["trusted-setup-attestation", "ceremony-attestation"]),
    expectedSchema: BSC_GROTH16_TRUSTED_SETUP_ATTESTATION_SCHEMA,
  }),
  Object.freeze({
    key: "reproducibleBuild",
    label: "reproducible build",
    optionNames: Object.freeze(["reproducible-build-attestation"]),
    expectedSchema: BSC_GROTH16_REPRODUCIBLE_BUILD_ATTESTATION_SCHEMA,
  }),
]);

const DEFAULT_GENERATED_MATERIAL_OUT =
  "output/sccp-bsc-production/groth16-material";
const DEFAULT_GROTH16_TOOLCHAIN_ROOT =
  "output/sccp-bsc-production/toolchain";
const DEFAULT_LOCAL_CIRCOM_RELATIVE_CANDIDATES = Object.freeze([
  "cargo/bin/circom",
  "node_modules/.bin/circom2",
]);
const DEFAULT_LOCAL_SNARKJS_RELATIVE_CANDIDATES = Object.freeze([
  "node_modules/.bin/snarkjs",
]);
const PRODUCTION_SNARKJS_R1CS_MIN_BYTES = 64 * 1024;
const PRODUCTION_SNARKJS_ZKEY_MIN_BYTES = 64 * 1024;
const PRODUCTION_FULL_SCCP_MIN_R1CS_CONSTRAINTS = 4096;
const COMMAND_PROBE_TIMEOUT_MS = 10_000;
const SNARKJS_R1CS_MAGIC = Object.freeze([0x72, 0x31, 0x63, 0x73]);
const SNARKJS_ZKEY_MAGIC = Object.freeze([0x7a, 0x6b, 0x65, 0x79]);
const SNARKJS_R1CS_HEADER_SECTION = 1;
const SNARKJS_R1CS_CONSTRAINTS_SECTION = 2;
const SNARKJS_R1CS_WIRE_MAP_SECTION = 3;
const SNARKJS_R1CS_REQUIRED_SECTIONS = Object.freeze([
  SNARKJS_R1CS_HEADER_SECTION,
  SNARKJS_R1CS_CONSTRAINTS_SECTION,
  SNARKJS_R1CS_WIRE_MAP_SECTION,
]);
const MAX_SNARKJS_SECTION_COUNT = 128;
const MAX_SNARKJS_CLI_R1CS_INFO_BYTES = 256 * 1024 * 1024;
const BN254_SCALAR_FIELD_MODULUS =
  21888242871839275222246405745257275088548364400416034343698204186575808495617n;
const BN254_BASE_FIELD_MODULUS =
  21888242871839275222246405745257275088696311157297823662689037894645226208583n;
const DECIMAL_WORD = /^(?:0|[1-9][0-9]*)$/u;
const SNARKJS_ZKEY_VERIFY_OK = "ZKey Ok!";
const BSC_GROTH16_SELF_TEST_SAMPLE_ID =
  "sccp-bsc-groth16-full-message-self-test-v1";
const BSC_GROTH16_SIGNAL_INPUT_NAMES = Object.freeze([
  "messageIdBits",
  "payloadHashBits",
  "targetDomainBits",
  "commitmentRootBits",
  "finalityHeightBits",
  "finalityBlockHashBits",
  "sourceDomainBits",
  "statementHashBits",
  "destinationBindingHashBits",
]);
const PRODUCTION_EVIDENCE_FORBIDDEN_WORDS =
  /\b(?:diagnostic|fixture|mock|placeholder|sample|stub|test-fixture|test-only)\b/iu;

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

function normalizeAttestationRole(value) {
  const direct = trim(value);
  if (BSC_GROTH16_ATTESTATION_ROLE_SPECS.some((spec) => spec.key === direct)) {
    return direct;
  }
  const normalized = trim(value)
    .replace(/[_\s]+/gu, "-")
    .toLowerCase();
  const aliases = new Map([
    ["semantic", "semanticSccpCircuit"],
    ["semantic-sccp", "semanticSccpCircuit"],
    ["semantic-sccp-circuit", "semanticSccpCircuit"],
    ["semantic-circuit", "semanticSccpCircuit"],
    ["semantics", "semanticSccpCircuit"],
    ["semantics-sccp-circuit", "semanticSccpCircuit"],
    ["security", "circuitSecurity"],
    ["circuit-security", "circuitSecurity"],
    ["circuit-audit", "circuitSecurity"],
    ["trusted-setup", "trustedSetup"],
    ["setup", "trustedSetup"],
    ["ceremony", "trustedSetup"],
    ["trusted-setup-ceremony", "trustedSetup"],
    ["reproducible", "reproducibleBuild"],
    ["reproducible-build", "reproducibleBuild"],
    ["rebuild", "reproducibleBuild"],
    ["independent-rebuild", "reproducibleBuild"],
    ["independent-reproducible-build", "reproducibleBuild"],
  ]);
  const roleKey = aliases.get(normalized);
  if (!roleKey) {
    throw new Error(
      "--role must be one of semanticSccpCircuit, circuitSecurity, trustedSetup, or reproducibleBuild.",
    );
  }
  return roleKey;
}

function attestationRoleSpec(roleKey) {
  const spec = BSC_GROTH16_ATTESTATION_ROLE_SPECS.find(
    (candidate) => candidate.key === roleKey,
  );
  if (!spec) {
    throw new Error(`unsupported attestation role: ${roleKey}`);
  }
  return spec;
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

const CLI_SIGNED_MATERIALIZE_OPTION_NAMES = Object.freeze([
  ...BSC_GROTH16_ATTESTATION_ROLE_SPECS.flatMap((spec) => spec.optionNames),
  "trusted-attestation-signer",
  "trusted-attestation-signer-fingerprint",
  "trusted-attestation-signers",
]);

function presentOptionNames(options, names) {
  return names.filter((name) => {
    const value = ownValue(options, name);
    return value !== undefined && value !== null && trim(value) !== "";
  });
}

function assertUnsignedCliMaterialize(options) {
  const supplied = presentOptionNames(options, CLI_SIGNED_MATERIALIZE_OPTION_NAMES);
  if (supplied.length === 0) {
    return;
  }
  throw new Error(
    "sccp_bsc_groth16_material.mjs materialize no longer accepts signed " +
      `attestation inputs on the CLI (${supplied.map((name) => `--${name}`).join(", ")}). ` +
      "Run materialize for unsigned candidate material, generate an " +
      "attestation-request package, then import signed role files through " +
      "finalize-attestations.",
  );
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

function bytesToHex(bytes) {
  return `0x${Buffer.from(bytes).toString("hex")}`;
}

async function fileSha256(pathName) {
  const hash = createHash("sha256");
  await new Promise((resolvePromise, rejectPromise) => {
    const stream = createReadStream(pathName);
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("error", rejectPromise);
    stream.on("end", resolvePromise);
  });
  return `0x${hash.digest("hex")}`;
}

const parseJsonStringToken = (text, start) => {
  let index = start + 1;
  while (index < text.length) {
    const char = text[index];
    if (char === '"') {
      return {
        value: JSON.parse(text.slice(start, index + 1)),
        end: index,
      };
    }
    if (char === "\\") {
      index += 2;
      continue;
    }
    index += 1;
  }
  return null;
};

const nextNonWhitespaceIndex = (text, start) => {
  let index = start;
  while (index < text.length && /\s/u.test(text[index])) {
    index += 1;
  }
  return index;
};

const duplicateJsonObjectKeyReason = (text, label) => {
  const stack = [];
  for (let index = 0; index < text.length; index += 1) {
    const char = text[index];
    if (char === '"') {
      const token = parseJsonStringToken(text, index);
      if (!token) {
        return "";
      }
      const current = stack.at(-1);
      if (
        current?.type === "object" &&
        current.expectingKey === true &&
        text[nextNonWhitespaceIndex(text, token.end + 1)] === ":"
      ) {
        if (current.keys.has(token.value)) {
          return `${label} contains a duplicate JSON object key.`;
        }
        current.keys.add(token.value);
        current.expectingKey = false;
      }
      index = token.end;
      continue;
    }
    if (char === "{") {
      stack.push({ type: "object", keys: new Set(), expectingKey: true });
      continue;
    }
    if (char === "[") {
      stack.push({ type: "array" });
      continue;
    }
    if (char === "}" || char === "]") {
      stack.pop();
      continue;
    }
    if (char === ",") {
      const current = stack.at(-1);
      if (current?.type === "object") {
        current.expectingKey = true;
      }
    }
  }
  return "";
};

function parseJsonWithoutDuplicateKeys(text, label = "JSON file") {
  const duplicateReason = duplicateJsonObjectKeyReason(text, label);
  if (duplicateReason) {
    throw new Error(duplicateReason);
  }
  return JSON.parse(text);
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
    return parseJsonWithoutDuplicateKeys(
      await readFile(resolve(pathName), "utf8"),
      label,
    );
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
  if (sourcePath === targetPath) {
    return targetPath;
  }
  await mkdir(dirname(targetPath), { recursive: true });
  await copyFile(sourcePath, targetPath);
  return targetPath;
}

async function canonicalFullMessageCircuitSourcePath() {
  const sourcePath = await assertReadableRegularFile(
    resolve(REPO_ROOT, DEFAULT_BSC_FULL_MESSAGE_CIRCUIT_SOURCE),
    "canonical BSC full-message circuit source",
  );
  const sourceText = await readFile(sourcePath, "utf8");
  if (sourceText !== generateBscFullMessageCircuitSource()) {
    throw new Error(
      `${DEFAULT_BSC_FULL_MESSAGE_CIRCUIT_SOURCE} has drifted from generateBscFullMessageCircuitSource(). Regenerate or review both before producing BSC Groth16 material.`,
    );
  }
  return sourcePath;
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

function normalizeSnarkjsZkeyVerifyResult(output) {
  const text = trim(output);
  return /ZKey Ok!/u.test(text) ? SNARKJS_ZKEY_VERIFY_OK : text;
}

function canonicalDecimalFieldWord(value, label, modulus = BN254_BASE_FIELD_MODULUS) {
  if (typeof value !== "string" || !DECIMAL_WORD.test(value)) {
    throw new Error(`${label} must be a canonical decimal BN254 field word.`);
  }
  const parsed = BigInt(value);
  if (parsed >= modulus) {
    throw new Error(`${label} must be a BN254 field element.`);
  }
  return parsed.toString(10);
}

function hex32Bytes(value, label) {
  return Buffer.from(normalizeHex32(value, label).slice(2), "hex");
}

function bigintFromBytes(bytes) {
  return BigInt(bytesToHex(bytes));
}

function byteBitsLittleEndian(byte) {
  return Array.from({ length: 8 }, (_, bit) => (byte >> bit) & 1);
}

function wordBitsLittleEndianByByte(bytes) {
  return Array.from(bytes).flatMap(byteBitsLittleEndian);
}

function bscGroth16SelfTestWord(profile, signalName) {
  return createHash("sha256")
    .update(`${BSC_GROTH16_SELF_TEST_SAMPLE_ID}:${profile.key}:${signalName}`)
    .digest();
}

function bscGroth16PublicSignal(labelHash, valueBytes) {
  const digest = keccak_256(Buffer.concat([
    hex32Bytes(labelHash, "BSC Groth16 signal label"),
    Buffer.from(valueBytes),
  ]));
  return (bigintFromBytes(digest) % BN254_SCALAR_FIELD_MODULUS).toString(10);
}

function bscGroth16SelfTestInput(profile) {
  const syntheticInputWords = Object.fromEntries(
    BSC_GROTH16_PUBLIC_SIGNAL_NAMES.map((signalName) => [
      signalName,
      bytesToHex(bscGroth16SelfTestWord(profile, signalName)),
    ]),
  );
  const publicSignals = BSC_GROTH16_PUBLIC_SIGNAL_NAMES.map(
    (signalName, index) =>
      bscGroth16PublicSignal(
        BSC_GROTH16_PUBLIC_SIGNAL_LABEL_HASHES[index],
        hex32Bytes(syntheticInputWords[signalName], `${signalName} self-test word`),
      ),
  );
  const input = { publicSignals };
  for (const [index, signalName] of BSC_GROTH16_PUBLIC_SIGNAL_NAMES.entries()) {
    input[BSC_GROTH16_SIGNAL_INPUT_NAMES[index]] = wordBitsLittleEndianByByte(
      hex32Bytes(syntheticInputWords[signalName], `${signalName} self-test word`),
    );
  }
  return {
    sampleId: BSC_GROTH16_SELF_TEST_SAMPLE_ID,
    syntheticInputWords,
    publicSignalWords: publicSignals,
    input,
  };
}

function nextDecimalFieldWord(word) {
  const value = BigInt(decimalWord(word, "BSC Groth16 self-test public signal"));
  return ((value + 1n) % BN254_SCALAR_FIELD_MODULUS).toString(10);
}

function cloneJson(value) {
  return JSON.parse(JSON.stringify(value));
}

function normalizeSnarkjsPublicSignals(value) {
  if (!Array.isArray(value)) {
    throw new Error("SnarkJS public signals must be a JSON array.");
  }
  if (value.length !== BSC_GROTH16_PUBLIC_SIGNAL_NAMES.length) {
    throw new Error("SnarkJS public signals must contain exactly 9 words.");
  }
  return value.map((entry, index) =>
    canonicalDecimalFieldWord(
      entry,
      `SnarkJS public signal ${index}`,
      BN254_SCALAR_FIELD_MODULUS,
    ),
  );
}

function snarkjsG1(point, label) {
  if (!Array.isArray(point) || point.length !== 3) {
    throw new Error(`${label} must be a SnarkJS projective G1 point.`);
  }
  const z = canonicalDecimalFieldWord(point[2], `${label}[2]`);
  if (z !== "1") {
    throw new Error(`${label}[2] must be the projective one coordinate.`);
  }
  return [
    canonicalDecimalFieldWord(point[0], `${label}[0]`),
    canonicalDecimalFieldWord(point[1], `${label}[1]`),
  ];
}

function snarkjsG2(point, label) {
  if (
    !Array.isArray(point) ||
    point.length !== 3 ||
    !Array.isArray(point[0]) ||
    !Array.isArray(point[1]) ||
    !Array.isArray(point[2]) ||
    point[0].length !== 2 ||
    point[1].length !== 2 ||
    point[2].length !== 2
  ) {
    throw new Error(`${label} must be a SnarkJS projective G2 point.`);
  }
  const z = [
    canonicalDecimalFieldWord(point[2][0], `${label}[2][0]`),
    canonicalDecimalFieldWord(point[2][1], `${label}[2][1]`),
  ];
  if (z[0] !== "1" || z[1] !== "0") {
    throw new Error(`${label}[2] must be the projective one coordinate.`);
  }
  return [
    canonicalDecimalFieldWord(point[0][0], `${label}[0][0]`),
    canonicalDecimalFieldWord(point[0][1], `${label}[0][1]`),
    canonicalDecimalFieldWord(point[1][0], `${label}[1][0]`),
    canonicalDecimalFieldWord(point[1][1], `${label}[1][1]`),
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

function explicitCommandValue(options, key) {
  const value =
    ownValue(options, key) ??
    process.env[key.toUpperCase().replace(/-/gu, "_")];
  return value === undefined || value === null || trim(value) === ""
    ? ""
    : trim(value);
}

function groth16ToolchainRoot(options = {}) {
  const raw =
    ownValue(options, "toolchain-root") ??
    ownValue(options, "groth16-toolchain-root") ??
    process.env.SCCP_BSC_GROTH16_TOOLCHAIN_ROOT;
  const explicit = raw !== undefined && raw !== null && trim(raw) !== "";
  const value = explicit ? trim(raw) : DEFAULT_GROTH16_TOOLCHAIN_ROOT;
  return {
    path: resolve(REPO_ROOT, value),
    explicit,
  };
}

function firstExistingToolchainCommand(options, relativeCandidates) {
  const root = groth16ToolchainRoot(options);
  for (const relativePath of relativeCandidates) {
    const candidate = resolve(root.path, relativePath);
    if (existsSync(candidate)) {
      return candidate;
    }
  }
  return root.explicit ? resolve(root.path, relativeCandidates[0]) : "";
}

function localToolchainCommand(options, key) {
  if (key === "circom-bin") {
    return firstExistingToolchainCommand(
      options,
      DEFAULT_LOCAL_CIRCOM_RELATIVE_CANDIDATES,
    );
  }
  if (key === "snarkjs-bin") {
    return firstExistingToolchainCommand(
      options,
      DEFAULT_LOCAL_SNARKJS_RELATIVE_CANDIDATES,
    );
  }
  return "";
}

function commandValue(options, key, fallback) {
  return (
    explicitCommandValue(options, key) ||
    localToolchainCommand(options, key) ||
    trim(fallback)
  );
}

function displayCommandValue(command) {
  const value = trim(command);
  return isAbsolute(value) ? repoRelativePath(value) : value;
}

function defaultMaterialOut(profile) {
  return resolve(DEFAULT_GENERATED_MATERIAL_OUT, profile.key);
}

function bscGroth16ArtifactPaths({ outDir, profile, circuitProfile }) {
  const artifactStem = trim(circuitProfile || BSC_FULL_SCCP_CIRCUIT_PROFILE);
  return {
    circuitSource: join(outDir, `${artifactStem}.circom`),
    r1cs: join(outDir, `${artifactStem}.r1cs`),
    witnessWasm: join(outDir, `${artifactStem}_js`, `${artifactStem}.wasm`),
    symbols: join(outDir, `${artifactStem}.sym`),
    provingKey: join(outDir, `${artifactStem}.final.zkey`),
    snarkjsVerificationKey: join(
      outDir,
      `${artifactStem}.snarkjs-verification-key.json`,
    ),
    bscVerifierKey: join(outDir, `${profile.key}-bsc-groth16-verifier-key.json`),
    manifest: join(outDir, `${profile.key}-bsc-groth16-material.manifest.json`),
    proofSelfTest: join(outDir, `${profile.key}-bsc-groth16-proof-self-test.json`),
  };
}

function commandProbe(command, args, { timeoutMs = COMMAND_PROBE_TIMEOUT_MS } = {}) {
  return new Promise((resolvePromise) => {
    const startedAt = Date.now();
    let settled = false;
    let stdout = "";
    let stderr = "";
    let timedOut = false;
    const finish = (result) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      resolvePromise({
        command,
        args,
        elapsedMs: Date.now() - startedAt,
        stdout: stdout.slice(-4096),
        stderr: stderr.slice(-4096),
        ...result,
      });
    };
    let child;
    const timer = setTimeout(() => {
      timedOut = true;
      if (child) {
        child.kill("SIGKILL");
      }
    }, timeoutMs);
    try {
      child = spawn(command, args, {
        stdio: ["ignore", "pipe", "pipe"],
      });
    } catch (error) {
      finish({
        ok: false,
        code: null,
        signal: null,
        error: error instanceof Error ? error.message : String(error),
      });
      return;
    }
    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString("utf8");
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString("utf8");
    });
    child.on("error", (error) => {
      finish({
        ok: false,
        code: null,
        signal: null,
        error: error instanceof Error ? error.message : String(error),
      });
    });
    child.on("close", (code, signal) => {
      finish({
        ok: code === 0,
        code,
        signal,
        error: timedOut
          ? `command probe timed out after ${timeoutMs}ms`
          : code === 0
            ? null
            : `command exited with code ${code}`,
      });
    });
  });
}

async function fileProbe(pathName) {
  const resolved = resolve(pathName);
  try {
    const info = await lstat(resolved);
    return {
      path: repoRelativePath(resolved),
      present: true,
      regularFile: info.isFile(),
      symbolicLink: info.isSymbolicLink(),
      size: info.size,
    };
  } catch (error) {
    if (error?.code === "ENOENT") {
      return {
        path: repoRelativePath(resolved),
        present: false,
        regularFile: false,
        symbolicLink: false,
        size: null,
      };
    }
    return {
      path: repoRelativePath(resolved),
      present: false,
      regularFile: false,
      symbolicLink: false,
      size: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
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

async function readExactly(fileHandle, length, position, label) {
  const buffer = Buffer.alloc(length);
  const { bytesRead } = await fileHandle.read(buffer, 0, length, position);
  if (bytesRead !== length) {
    throw new Error(`${label} is truncated`);
  }
  return buffer;
}

async function readSnarkjsSectionTable(pathName, label, magic) {
  const fileHandle = await open(pathName, "r");
  try {
    const stat = await fileHandle.stat();
    if (stat.size < 12) {
      throw new Error(`${label} SnarkJS section table is truncated`);
    }
    const header = await readExactly(fileHandle, 12, 0, label);
    const hasMagic = magic.every((byte, index) => header[index] === byte);
    if (!hasMagic) {
      throw new Error(`${label} must start with SnarkJS ${label} magic bytes`);
    }
    const version = u32le(header, 4);
    if (version < 1 || version > 2) {
      throw new Error(`${label} SnarkJS version is unsupported`);
    }
    const sectionCount = u32le(header, 8);
    if (sectionCount < 1 || sectionCount > MAX_SNARKJS_SECTION_COUNT) {
      throw new Error(`${label} SnarkJS section count is invalid`);
    }
    const parseSections = async (layout) => {
      let descriptorOffset = 12;
      let contentOffset =
        layout === "grouped" ? 12 + sectionCount * 12 : null;
      if (layout === "grouped" && contentOffset > stat.size) {
        throw new Error(`${label} SnarkJS section table is truncated`);
      }
      const sections = new Map();
      for (let index = 0; index < sectionCount; index += 1) {
        const descriptor = await readExactly(
          fileHandle,
          12,
          descriptorOffset,
          `${label} SnarkJS section table`,
        );
        descriptorOffset += 12;
        const sectionId = u32le(descriptor, 0);
        const sectionSize = u64leSafe(descriptor, 4);
        if (sectionId === 0) {
          throw new Error(`${label} SnarkJS section id must be non-zero`);
        }
        if (sectionSize === null || sectionSize <= 0) {
          throw new Error(`${label} SnarkJS section size is invalid`);
        }
        const payloadOffset =
          layout === "grouped" ? contentOffset : descriptorOffset;
        if (sectionSize > stat.size - payloadOffset) {
          throw new Error(`${label} SnarkJS section exceeds file size`);
        }
        const entry = {
          id: sectionId,
          offset: payloadOffset,
          size: sectionSize,
        };
        if (sections.has(sectionId)) {
          const current = sections.get(sectionId);
          sections.set(
            sectionId,
            Array.isArray(current) ? [...current, entry] : [current, entry],
          );
        } else {
          sections.set(sectionId, entry);
        }
        if (layout === "grouped") {
          contentOffset += sectionSize;
        } else {
          descriptorOffset = payloadOffset + sectionSize;
        }
      }
      const finalOffset =
        layout === "grouped" ? contentOffset : descriptorOffset;
      if (finalOffset !== stat.size) {
        throw new Error(
          `${label} SnarkJS section table does not consume the full file`,
        );
      }
      return { version, sectionCount, sections, size: stat.size, layout };
    };
    try {
      return await parseSections("interleaved");
    } catch (interleavedError) {
      try {
        return await parseSections("grouped");
      } catch (_groupedError) {
        throw interleavedError;
      }
    }
  } finally {
    await fileHandle.close();
  }
}

async function snarkjsArtifactBlockers(pathName, label, magic, minBytes) {
  const { size } = await lstat(pathName);
  const blockers = [];
  if (size < minBytes) {
    blockers.push(`${label} must be at least ${minBytes} bytes for production material`);
  }
  try {
    await readSnarkjsSectionTable(pathName, label, magic);
  } catch (error) {
    blockers.push(error instanceof Error ? error.message : String(error));
  }
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

async function readSnarkjsR1csHeader(pathName) {
  const table = await readSnarkjsSectionTable(pathName, "R1CS", SNARKJS_R1CS_MAGIC);
  for (const sectionId of SNARKJS_R1CS_REQUIRED_SECTIONS) {
    if (!table.sections.has(sectionId)) {
      throw new Error(`R1CS SnarkJS section ${sectionId} is required`);
    }
  }
  const headerSection = table.sections.get(SNARKJS_R1CS_HEADER_SECTION);
  if (Array.isArray(headerSection)) {
    throw new Error("R1CS header section must be unique");
  }
  const fileHandle = await open(pathName, "r");
  try {
    const fixedPrefix = await readExactly(
      fileHandle,
      4,
      headerSection.offset,
      "R1CS header section",
    );
    const n8 = u32le(fixedPrefix, 0);
    if (n8 <= 0 || n8 > 256 || n8 % 8 !== 0) {
      throw new Error("R1CS header field size is invalid");
    }
    const expectedHeaderSize = 32 + n8;
    if (headerSection.size !== expectedHeaderSize) {
      throw new Error("R1CS header section size is invalid");
    }
    const header = await readExactly(
      fileHandle,
      expectedHeaderSize,
      headerSection.offset,
      "R1CS header section",
    );
    return {
      source: "binary-header",
      n8,
      nVars: u32le(header, 4 + n8),
      nOutputs: u32le(header, 8 + n8),
      nPubInputs: u32le(header, 12 + n8),
      nPrvInputs: u32le(header, 16 + n8),
      nLabels: u64leSafe(header, 20 + n8),
      nConstraints: u32le(header, 28 + n8),
      sectionCount: table.sectionCount,
      fileSize: table.size,
    };
  } finally {
    await fileHandle.close();
  }
}

function applyR1csSelfCheckCounts(checks, blockers, parsed, circuitProfile, source) {
  checks.r1csConstraintCount = parsed.constraintCount;
  checks.r1csPublicInputCount = parsed.publicInputCount;
  checks.r1csInfo = true;
  checks.r1csInfoSource = source;
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
}

async function snarkjsMaterialSelfCheck({
  snarkjsBin,
  r1csPath,
  zkeyPath,
  ptauPath = null,
  profile,
  verifierKeyHash,
  circuitProfile,
}) {
  const checks = {
    snarkjsBinary: snarkjsBin,
    r1csInfo: false,
    r1csInfoSource: null,
    r1csInfoError: null,
    r1csConstraintCount: null,
    r1csPublicInputCount: null,
    r1csBinaryHeader: null,
    zkeyVerify: false,
    zkeyVerifyResult: null,
    zkeyVerifyError: null,
    zkeyVerificationKeyExport: false,
    verifierKeyHashMatches: false,
    exportedVerifierKeyHash: null,
  };
  const blockers = [];
  let snarkjsR1csInfoError = null;
  const { size: r1csSize } = await lstat(r1csPath);
  if (r1csSize <= MAX_SNARKJS_CLI_R1CS_INFO_BYTES) {
    try {
      const info = await runCommand(snarkjsBin, ["r1cs", "info", r1csPath]);
      const parsed = parseSnarkjsR1csInfo(`${info.stdout}\n${info.stderr}`);
      applyR1csSelfCheckCounts(
        checks,
        blockers,
        parsed,
        circuitProfile,
        "snarkjs-cli",
      );
    } catch (error) {
      snarkjsR1csInfoError = error instanceof Error ? error.message : String(error);
    }
  } else {
    snarkjsR1csInfoError = `skipped snarkjs r1cs info for ${r1csSize} byte R1CS; using bounded binary header parser`;
  }
  if (!checks.r1csInfo) {
    try {
      const header = await readSnarkjsR1csHeader(r1csPath);
      checks.r1csBinaryHeader = header;
      applyR1csSelfCheckCounts(
        checks,
        blockers,
        {
          constraintCount: header.nConstraints,
          publicInputCount: header.nPubInputs,
        },
        circuitProfile,
        snarkjsR1csInfoError ? "binary-header-fallback" : "binary-header",
      );
      checks.r1csInfoError = snarkjsR1csInfoError;
    } catch (error) {
      const headerError = error instanceof Error ? error.message : String(error);
      checks.r1csInfoError = snarkjsR1csInfoError
        ? `${snarkjsR1csInfoError}; binary header fallback failed: ${headerError}`
        : headerError;
      blockers.push(`SnarkJS R1CS self-check failed: ${checks.r1csInfoError}`);
    }
  }

  if (ptauPath) {
    try {
      const verify = await runCommand(snarkjsBin, [
        "zkey",
        "verify",
        r1csPath,
        ptauPath,
        zkeyPath,
      ]);
      checks.zkeyVerifyResult = normalizeSnarkjsZkeyVerifyResult(
        `${verify.stdout}\n${verify.stderr}`,
      );
      checks.zkeyVerify = checks.zkeyVerifyResult === SNARKJS_ZKEY_VERIFY_OK;
      if (!checks.zkeyVerify) {
        blockers.push(
          `SnarkJS zkey verify self-check must report ${SNARKJS_ZKEY_VERIFY_OK}.`,
        );
      }
    } catch (error) {
      checks.zkeyVerifyError = error instanceof Error ? error.message : String(error);
      blockers.push(`SnarkJS zkey verify self-check failed: ${checks.zkeyVerifyError}`);
    }
  } else if (circuitProfile === BSC_FULL_SCCP_CIRCUIT_PROFILE) {
    blockers.push(
      "SnarkJS zkey verify self-check requires a Powers of Tau artifact.",
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

function integerMatchesBlocker(record, keys, expected, label) {
  if (!Number.isSafeInteger(expected)) {
    return `${label} expected value is unavailable`;
  }
  return integerEqualsBlocker(record, keys, expected, label);
}

function stringMatchesBlocker(record, keys, expected, label) {
  if (typeof expected !== "string" || trim(expected) === "") {
    return `${label} expected value is unavailable`;
  }
  return stringEqualsBlocker(record, keys, expected, label);
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

function transcriptArrayOrCountAtLeastBlocker(
  record,
  arrayKeys,
  countKeys,
  minimum,
  label,
) {
  for (const key of arrayKeys) {
    const value = attestationValue(record, key);
    if (Array.isArray(value)) {
      const distinct = new Set(
        value.map((entry) => trim(entry)).filter(Boolean),
      );
      return distinct.size >= minimum
        ? ""
        : `${label} must record at least ${minimum}`;
    }
  }
  const count = Number(attestationValue(record, countKeys));
  if (Number.isSafeInteger(count)) {
    return count >= minimum ? "" : `${label} must record at least ${minimum}`;
  }
  return `${label} is required`;
}

function productionEvidenceTextBlockers(value, label, path = "") {
  const blockers = [];
  if (typeof value === "string") {
    const scanValue =
      path.endsWith(".path") || path === ".path"
        ? basename(win32.basename(value))
        : value;
    if (PRODUCTION_EVIDENCE_FORBIDDEN_WORDS.test(scanValue)) {
      blockers.push(
        `${label}${path} must not reference diagnostic, fixture, mock, placeholder, sample, stub, or test-only material`,
      );
    }
    return blockers;
  }
  if (Array.isArray(value)) {
    for (const [index, entry] of value.entries()) {
      blockers.push(
        ...productionEvidenceTextBlockers(entry, label, `${path}[${index}]`),
      );
    }
    return blockers;
  }
  if (isRecord(value)) {
    for (const [key, entry] of Object.entries(value)) {
      blockers.push(
        ...productionEvidenceTextBlockers(
          entry,
          label,
          path ? `${path}.${key}` : `.${key}`,
        ),
      );
    }
  }
  return blockers;
}

function transcriptMaterializeCommandBlockers(record, label) {
  const commands = attestationValue(record, ["commands", "commandLog", "command_log"]);
  if (commands === undefined || commands === null) {
    return [];
  }
  if (!Array.isArray(commands)) {
    return [`${label} commands must be an array when present`];
  }
  const blockers = [];
  for (const [index, command] of commands.entries()) {
    if (typeof command !== "string") {
      blockers.push(`${label} commands[${index}] must be a string`);
      continue;
    }
    if (
      !/sccp_bsc_groth16_material\.mjs\s+materialize/u.test(command) &&
      !/sccp_bsc_taira_xor_deploy\.mjs\s+groth16-material\s+materialize/u.test(command)
    ) {
      continue;
    }
    if (!/\s--ptau\s+\S+/u.test(command)) {
      blockers.push(`${label} commands[${index}] materialize command must include --ptau`);
    }
    if (!/\s--trusted-setup-transcript\s+\S+/u.test(command)) {
      blockers.push(
        `${label} commands[${index}] materialize command must include --trusted-setup-transcript`,
      );
    }
    if (!/\s--reproducible-build-transcript\s+\S+/u.test(command)) {
      blockers.push(
        `${label} commands[${index}] materialize command must include --reproducible-build-transcript`,
      );
    }
    if (/\s--(?:semantic|circuit-security|trusted-setup|reproducible-build)-attestation\s/u.test(command)) {
      blockers.push(
        `${label} commands[${index}] materialize command must not pass signed attestation files directly`,
      );
    }
  }
  return blockers;
}

async function sourceBuildTranscriptBlockers(record, label, pathName) {
  const sourceBuildTranscript = attestationValue(record, [
    "sourceBuildTranscript",
    "source_build_transcript",
  ]);
  if (sourceBuildTranscript === undefined || sourceBuildTranscript === null) {
    return [];
  }
  if (!isRecord(sourceBuildTranscript)) {
    return [`${label} sourceBuildTranscript must be an object when present`];
  }
  const sourcePath = trim(attestationValue(sourceBuildTranscript, ["path"]));
  if (!sourcePath) {
    return [`${label} sourceBuildTranscript.path is required`];
  }
  let expectedSha256;
  try {
    expectedSha256 = normalizeHex32(
      attestationValue(sourceBuildTranscript, ["sha256", "hash"]),
      `${label} sourceBuildTranscript.sha256`,
    );
  } catch (error) {
    return [error instanceof Error ? error.message : String(error)];
  }
  const candidates = [
    sourcePath,
    ...(isAbsolute(sourcePath) ? [] : [resolve(dirname(pathName), sourcePath)]),
  ];
  let lastError = null;
  for (const candidate of [...new Set(candidates)]) {
    try {
      const actualSha256 = await fileSha256(candidate);
      return actualSha256 === expectedSha256
        ? []
        : [
            `${label} sourceBuildTranscript.sha256 must match ${actualSha256}`,
          ];
    } catch (error) {
      lastError = error;
    }
  }
  return [
    `${label} sourceBuildTranscript.path could not be read: ${
      lastError instanceof Error ? lastError.message : String(lastError)
    }`,
  ];
}

function materialManifestReferenceBlockers(manifest, label = "material manifest") {
  if (!isRecord(manifest)) {
    return [];
  }
  const artifacts = ownValue(manifest, "artifacts");
  const attestations = ownValue(manifest, "attestations");
  return [
    ...(isRecord(artifacts)
      ? productionEvidenceTextBlockers(artifacts, `${label} artifacts`)
      : []),
    ...(isRecord(attestations)
      ? productionEvidenceTextBlockers(attestations, `${label} attestations`)
      : []),
  ];
}

function attestationBodyAllowedFields(expectedSchema) {
  const common = [
    "schema",
    "routeId",
    "route_id",
    "assetKey",
    "asset_key",
    "bscNetwork",
    "bsc_network",
    "network",
    "chain",
    "chainIdHex",
    "chain_id_hex",
    "networkIdHex",
    "network_id_hex",
    "proofBackend",
    "proof_backend",
    "proofFamily",
    "proof_family",
    "circuitProfile",
    "circuit_profile",
    "publicInputCount",
    "public_input_count",
    "publicSignalNames",
    "public_signal_names",
    "verifierKeyHash",
    "verifier_key_hash",
    "circuitSourceSha256",
    "circuit_source_sha256",
    "r1csSha256",
    "r1cs_sha256",
    "proofArtifactHash",
    "proof_artifact_hash",
    "powersOfTauSha256",
    "powers_of_tau_sha256",
    "ptauSha256",
    "ptau_sha256",
    "provingKeySha256",
    "proving_key_sha256",
    "provingKeyHash",
    "proving_key_hash",
    "snarkjsVerificationKeySha256",
    "snarkjs_verification_key_sha256",
    "bscVerifierKeySha256",
    "bsc_verifier_key_sha256",
    "signature",
  ];
  const bySchema = {
    [BSC_GROTH16_SEMANTIC_ATTESTATION_SCHEMA]: [
      "fullSccpMessageSemantics",
      "full_sccp_message_semantics",
      "sourceFinalitySemantics",
      "source_finality_semantics",
      "destinationBindingSemantics",
      "destination_binding_semantics",
      "publicSignalDerivationSemantics",
      "public_signal_derivation_semantics",
      "negativeCaseCoverage",
      "negative_case_coverage",
    ],
    [BSC_GROTH16_CIRCUIT_SECURITY_ATTESTATION_SCHEMA]: [
      "auditResult",
      "audit_result",
      "approved",
      "productionApproved",
      "production_approved",
      "criticalFindings",
      "critical_findings",
      "highFindings",
      "high_findings",
      "unresolvedFindings",
      "unresolved_findings",
    ],
    [BSC_GROTH16_TRUSTED_SETUP_ATTESTATION_SCHEMA]: [
      "ceremonyResult",
      "ceremony_result",
      "localSingleContributor",
      "local_single_contributor",
      "minimumContributors",
      "minimum_contributors",
      "toxicWasteDestroyed",
      "toxic_waste_destroyed",
      "contributionTranscriptSha256",
      "contribution_transcript_sha256",
    ],
    [BSC_GROTH16_REPRODUCIBLE_BUILD_ATTESTATION_SCHEMA]: [
      "reproducible",
      "reproducibleBuild",
      "reproducible_build",
      "independentRebuilders",
      "independent_rebuilders",
      "buildTranscriptSha256",
      "build_transcript_sha256",
      "toolchainSha256",
      "toolchain_sha256",
      "r1csInfoSource",
      "r1cs_info_source",
      "r1csPublicInputCount",
      "r1cs_public_input_count",
      "r1csConstraintCount",
      "r1cs_constraint_count",
      "zkeyVerify",
      "zkey_verify",
      "zkeyVerifyResult",
      "zkey_verify_result",
      "zkeyVerificationKeyExport",
      "zkey_verification_key_export",
      "verifierKeyHashMatches",
      "verifier_key_hash_matches",
      "exportedVerifierKeyHash",
      "exported_verifier_key_hash",
    ],
  };
  return new Set([...common, ...(bySchema[expectedSchema] ?? [])]);
}

function attestationBodyUnknownFieldBlockers(record, expectedSchema, label) {
  const allowed = attestationBodyAllowedFields(expectedSchema);
  return Object.keys(record)
    .filter((key) => !allowed.has(key))
    .map((key) => `${label} contains unknown field: ${key}`);
}

function optionalBooleanFalseBlocker(record, keys, label) {
  const value = attestationValue(record, keys);
  return value === undefined ? "" : value === false ? "" : `${label} must be false`;
}

function optionalBooleanTrueBlocker(record, keys, label) {
  const value = attestationValue(record, keys);
  return value === undefined ? "" : value === true ? "" : `${label} must be true`;
}

function optionalStringMatchesBlocker(record, keys, expected, label) {
  return attestationValue(record, keys) === undefined
    ? ""
    : stringMatchesBlocker(record, keys, expected, label);
}

function optionalIntegerMatchesBlocker(record, keys, expected, label) {
  return attestationValue(record, keys) === undefined
    ? ""
    : integerMatchesBlocker(record, keys, expected, label);
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
    ...attestationBodyUnknownFieldBlockers(record, expectedSchema, `${label} attestation`),
    ...productionEvidenceTextBlockers(record, `${label} attestation`),
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
      ["proofBackend", "proof_backend"],
      BSC_EVM_GROTH16_BACKEND,
      `${label} proofBackend`,
    ),
    stringEqualsBlocker(
      record,
      ["proofFamily", "proof_family"],
      SCCP_PROOF_FAMILY_STARK_FRI,
      `${label} proofFamily`,
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
  if (context.artifacts.powersOfTau) {
    blockers.push(
      hashEqualsBlocker(
        record,
        [
          "powersOfTauSha256",
          "powers_of_tau_sha256",
          "ptauSha256",
          "ptau_sha256",
        ],
        context.artifacts.powersOfTau.sha256,
        `${label} powersOfTauSha256`,
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
  const transcriptArtifact = context.artifacts.trustedSetupTranscript;
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
          transcriptArtifact
            ? hashEqualsBlocker(
                record,
                ["contributionTranscriptSha256", "contribution_transcript_sha256"],
                transcriptArtifact.sha256,
                `${label} contributionTranscriptSha256`,
              )
            : `${label} transcript artifact is required`,
        ].filter(Boolean)
      : []),
  ];
}

function validateReproducibleBuildAttestation(entry, context) {
  const label = "reproducible build";
  const record = entry?.record;
  const snarkjsSelfCheck = context.selfChecks?.snarkjs;
  const transcriptArtifact = context.artifacts.reproducibleBuildTranscript;
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
          transcriptArtifact
            ? hashEqualsBlocker(
                record,
                ["buildTranscriptSha256", "build_transcript_sha256"],
                transcriptArtifact.sha256,
                `${label} buildTranscriptSha256`,
              )
            : `${label} transcript artifact is required`,
          hashPresentBlocker(
            record,
            ["toolchainSha256", "toolchain_sha256"],
            `${label} toolchainSha256`,
          ),
          stringMatchesBlocker(
            record,
            ["r1csInfoSource", "r1cs_info_source"],
            snarkjsSelfCheck?.r1csInfoSource,
            `${label} r1csInfoSource`,
          ),
          integerMatchesBlocker(
            record,
            ["r1csPublicInputCount", "r1cs_public_input_count"],
            snarkjsSelfCheck?.r1csPublicInputCount,
            `${label} r1csPublicInputCount`,
          ),
          integerMatchesBlocker(
            record,
            ["r1csConstraintCount", "r1cs_constraint_count"],
            snarkjsSelfCheck?.r1csConstraintCount,
            `${label} r1csConstraintCount`,
          ),
          booleanTrueBlocker(
            record,
            ["zkeyVerify", "zkey_verify"],
            `${label} zkeyVerify`,
          ),
          stringMatchesBlocker(
            record,
            ["zkeyVerifyResult", "zkey_verify_result"],
            snarkjsSelfCheck?.zkeyVerifyResult,
            `${label} zkeyVerifyResult`,
          ),
          booleanTrueBlocker(
            record,
            ["zkeyVerificationKeyExport", "zkey_verification_key_export"],
            `${label} zkeyVerificationKeyExport`,
          ),
          booleanTrueBlocker(
            record,
            ["verifierKeyHashMatches", "verifier_key_hash_matches"],
            `${label} verifierKeyHashMatches`,
          ),
          hashEqualsBlocker(
            record,
            ["exportedVerifierKeyHash", "exported_verifier_key_hash"],
            context.verifierKeyHash,
            `${label} exportedVerifierKeyHash`,
          ),
        ].filter(Boolean)
      : []),
  ];
}

async function validateTrustedSetupTranscript(pathName) {
  const label = "trusted setup transcript";
  if (!pathName) {
    return [`missing ${label} artifact`];
  }
  let record;
  try {
    record = await readJson(pathName, label);
  } catch (error) {
    return [error instanceof Error ? error.message : String(error)];
  }
  if (!isRecord(record)) {
    return [`${label} must be a JSON object`];
  }
  const secretReason = unsafeSecretReason(record, label);
  if (secretReason) {
    return [secretReason];
  }
  const phase1 = isRecord(record.phase1) ? record.phase1 : null;
  const snarkjsPowersOfTauVerify = phase1
    ? attestationValue(phase1, [
        "snarkjsPowersOfTauVerify",
        "snarkjs_powers_of_tau_verify",
      ])
    : undefined;
  const phase2 = isRecord(record.phase2) ? record.phase2 : null;
  return [
    ...productionEvidenceTextBlockers(record, label),
    ...transcriptMaterializeCommandBlockers(record, label),
    ...(await sourceBuildTranscriptBlockers(record, label, pathName)),
    transcriptArrayOrCountAtLeastBlocker(
      record,
      ["contributors", "participants", "contributions"],
      [
        "minimumContributors",
        "minimum_contributors",
        "minimumContributorsObserved",
        "minimum_contributors_observed",
      ],
      2,
      `${label} contributors`,
    ),
    booleanFalseBlocker(
      record,
      ["localSingleContributor", "local_single_contributor"],
      `${label} localSingleContributor`,
    ),
    booleanTrueBlocker(
      record,
      ["toxicWasteDestroyed", "toxic_waste_destroyed"],
      `${label} toxicWasteDestroyed`,
    ),
    stringEqualsBlocker(
      record,
      ["ceremonyResult", "ceremony_result"],
      "pass",
      `${label} ceremonyResult`,
    ),
    phase1 ? "" : `${label} phase1 block is required`,
    isRecord(snarkjsPowersOfTauVerify)
      ? booleanTrueBlocker(
          snarkjsPowersOfTauVerify,
          ["completed"],
          `${label} snarkjsPowersOfTauVerify.completed`,
        )
      : `${label} snarkjsPowersOfTauVerify block is required`,
    phase2 ? "" : `${label} phase2 block is required`,
    phase2
      ? stringEqualsBlocker(
          phase2,
          ["snarkjsZkeyVerify", "snarkjs_zkey_verify"],
          "ZKey Ok!",
          `${label} snarkjsZkeyVerify`,
        )
      : "",
  ].filter(Boolean);
}

async function validateReproducibleBuildTranscript(pathName, selfCheck) {
  const label = "reproducible build transcript";
  if (!pathName) {
    return [`missing ${label} artifact`];
  }
  let record;
  try {
    record = await readJson(pathName, label);
  } catch (error) {
    return [error instanceof Error ? error.message : String(error)];
  }
  if (!isRecord(record)) {
    return [`${label} must be a JSON object`];
  }
  const secretReason = unsafeSecretReason(record, label);
  if (secretReason) {
    return [secretReason];
  }
  return [
    ...productionEvidenceTextBlockers(record, label),
    ...transcriptMaterializeCommandBlockers(record, label),
    ...(await sourceBuildTranscriptBlockers(record, label, pathName)),
    transcriptArrayOrCountAtLeastBlocker(
      record,
      ["independentRebuilders", "independent_rebuilders", "rebuilders"],
      [
        "independentRebuilderCount",
        "independent_rebuilder_count",
        "independentRebuildersObserved",
        "independent_rebuilders_observed",
      ],
      2,
      `${label} independentRebuilders`,
    ),
    booleanTrueBlocker(
      record,
      ["reproducible", "reproducibleBuildComplete", "reproducible_build_complete"],
      `${label} reproducible`,
    ),
    optionalStringMatchesBlocker(
      record,
      ["r1csInfoSource", "r1cs_info_source"],
      selfCheck?.r1csInfoSource,
      `${label} r1csInfoSource`,
    ),
    optionalIntegerMatchesBlocker(
      record,
      ["r1csPublicInputCount", "r1cs_public_input_count"],
      selfCheck?.r1csPublicInputCount,
      `${label} r1csPublicInputCount`,
    ),
    optionalIntegerMatchesBlocker(
      record,
      ["r1csConstraintCount", "r1cs_constraint_count"],
      selfCheck?.r1csConstraintCount,
      `${label} r1csConstraintCount`,
    ),
    booleanTrueBlocker(
      record,
      ["zkeyVerify", "zkey_verify"],
      `${label} zkeyVerify`,
    ),
    stringMatchesBlocker(
      record,
      ["zkeyVerifyResult", "zkey_verify_result"],
      selfCheck?.zkeyVerifyResult,
      `${label} zkeyVerifyResult`,
    ),
  ].filter(Boolean);
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
  trustedSetupTranscriptPath = null,
  reproducibleBuildTranscriptPath = null,
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
    ...(trustedSetupTranscriptPath
      ? { trustedSetupTranscript: await artifactRecord(trustedSetupTranscriptPath) }
      : {}),
    ...(reproducibleBuildTranscriptPath
      ? {
          reproducibleBuildTranscript: await artifactRecord(
            reproducibleBuildTranscriptPath,
          ),
        }
      : {}),
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
    ptauPath,
    profile,
    verifierKeyHash: verifierMaterial.verifierKeyHash,
    circuitProfile,
  });
  const attestationContext = {
    ...context,
    selfChecks: {
      snarkjs: selfCheck.checks,
      ...(sourceCheck ? { circuitSource: sourceCheck.checks } : {}),
    },
  };
  const attestationValidationBlockers = validateAttestationsForMaterial(
    attestations,
    attestationContext,
    trustedSignerFingerprints,
  );
  const publicAttestations = publicAttestationReferences(attestations);
  const referenceLabelBlockers = materialManifestReferenceBlockers({
    artifacts,
    attestations: publicAttestations,
  });
  const transcriptValidationBlockers =
    circuitProfile === BSC_FULL_SCCP_CIRCUIT_PROFILE
      ? [
          ...(await validateTrustedSetupTranscript(trustedSetupTranscriptPath)),
          ...(await validateReproducibleBuildTranscript(
            reproducibleBuildTranscriptPath,
            selfCheck.checks,
          )),
        ]
      : [];
  const productionBlockers = productionBlockersForMaterial({
    circuitProfile,
    localPtau,
    localPhase2,
    attestations,
    attestationValidationBlockers,
  }).concat(
    artifactBlockers,
    selfCheck.blockers,
    transcriptValidationBlockers,
    referenceLabelBlockers,
  );
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
    attestations: publicAttestations,
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
  const resolvedCircuitSource =
    externalCircuitSource ??
    (circuitProfile === BSC_FULL_SCCP_CIRCUIT_PROFILE
      ? await canonicalFullMessageCircuitSourcePath()
      : null);
  if (resolvedCircuitSource) {
    await copyPublicFile(
      resolvedCircuitSource,
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
  const trustedSetupTranscriptInput = optionalPath(options, [
    "trusted-setup-transcript",
    "contribution-transcript",
    "ceremony-transcript",
  ]);
  const trustedSetupTranscriptPath = trustedSetupTranscriptInput
    ? await copyPublicFile(
        trustedSetupTranscriptInput,
        join(outDir, basename(trustedSetupTranscriptInput)),
        "trusted setup transcript",
      )
    : null;
  const reproducibleBuildTranscriptInput = optionalPath(options, [
    "reproducible-build-transcript",
    "build-transcript",
  ]);
  const reproducibleBuildTranscriptPath = reproducibleBuildTranscriptInput
    ? await copyPublicFile(
        reproducibleBuildTranscriptInput,
        join(outDir, basename(reproducibleBuildTranscriptInput)),
        "reproducible build transcript",
      )
    : null;
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
    trustedSetupTranscriptPath,
    reproducibleBuildTranscriptPath,
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
  const ptauInput = optionalPath(options, ["ptau", "powers-of-tau", "powersoftau"]);
  const ptauPath = ptauInput
    ? await copyPublicFile(
        ptauInput,
        join(outDir, basename(ptauInput)),
        "Powers of Tau file",
      )
    : null;
  const circuitSourceInput = optionalPath(options, "circuit-source");
  const circuitProfile = trim(
    ownValue(options, "circuit-profile") ?? BSC_FULL_SCCP_CIRCUIT_PROFILE,
  );
  const resolvedCircuitSource =
    circuitSourceInput ??
    (circuitProfile === BSC_FULL_SCCP_CIRCUIT_PROFILE
      ? await canonicalFullMessageCircuitSourcePath()
      : null);
  const circuitSourcePath = resolvedCircuitSource
    ? await copyPublicFile(
        resolvedCircuitSource,
        join(outDir, basename(resolvedCircuitSource)),
        "circuit source",
      )
    : null;
  const trustedSetupTranscriptInput = optionalPath(options, [
    "trusted-setup-transcript",
    "contribution-transcript",
    "ceremony-transcript",
  ]);
  const trustedSetupTranscriptPath = trustedSetupTranscriptInput
    ? await copyPublicFile(
        trustedSetupTranscriptInput,
        join(outDir, basename(trustedSetupTranscriptInput)),
        "trusted setup transcript",
      )
    : null;
  const reproducibleBuildTranscriptInput = optionalPath(options, [
    "reproducible-build-transcript",
    "build-transcript",
  ]);
  const reproducibleBuildTranscriptPath = reproducibleBuildTranscriptInput
    ? await copyPublicFile(
        reproducibleBuildTranscriptInput,
        join(outDir, basename(reproducibleBuildTranscriptInput)),
        "reproducible build transcript",
      )
    : null;
  const attestations = await buildAttestationReferences(options);
  const trustedSignerFingerprints = parseTrustedSignerFingerprints(options);
  const snarkjsBin = commandValue(options, "snarkjs-bin", "snarkjs");
  return generateMaterialFromVerificationKey({
    profile,
    outDir,
    snarkjsVerifierKeyPath,
    r1csPath,
    zkeyPath,
    ptauPath,
    circuitSourcePath,
    trustedSetupTranscriptPath,
    reproducibleBuildTranscriptPath,
    localPtau: false,
    localPhase2: false,
    circuitProfile,
    attestations,
    trustedSignerFingerprints,
    snarkjsBin,
  });
}

function normalizeManifestHash(value, label) {
  return normalizeHex32(value, label);
}

function materialManifestArtifact(manifest, key, label = key) {
  const artifacts = ownValue(manifest, "artifacts");
  const artifact = isRecord(artifacts) ? ownValue(artifacts, key) : null;
  if (!isRecord(artifact)) {
    throw new Error(`material manifest ${label} artifact is required.`);
  }
  const artifactPath = trim(ownValue(artifact, "path"));
  if (!artifactPath) {
    throw new Error(`material manifest ${label} artifact path is required.`);
  }
  return {
    path: artifactPath,
    sha256: normalizeManifestHash(
      ownValue(artifact, "sha256"),
      `material manifest ${label} artifact sha256`,
    ),
  };
}

async function resolveManifestArtifactPath(manifestPath, artifact, label) {
  const artifactPath = trim(artifact.path);
  const candidates = isAbsolute(artifactPath)
    ? [resolve(artifactPath)]
    : [
        resolve(dirname(manifestPath), artifactPath),
        resolve(REPO_ROOT, artifactPath),
        resolve(process.cwd(), artifactPath),
      ];
  const uniqueCandidates = [...new Set(candidates)];
  for (const candidate of uniqueCandidates) {
    try {
      return await assertReadableRegularFile(candidate, label);
    } catch (error) {
      if (error?.code !== "ENOENT") {
        throw error;
      }
    }
  }
  throw new Error(
    `${label} could not be resolved from manifest artifact path ${artifactPath}.`,
  );
}

async function readManifestJsonArtifact(manifestPath, artifact, label) {
  const resolved = await resolveManifestArtifactPath(manifestPath, artifact, label);
  const actualHash = await fileSha256(resolved);
  if (actualHash !== artifact.sha256) {
    throw new Error(`${label} sha256 must match material manifest.`);
  }
  const record = await readJson(resolved, label);
  const secretReason = unsafeSecretReason(record, label);
  if (secretReason) {
    throw new Error(secretReason);
  }
  return { path: resolved, record };
}

async function resolveManifestArtifactFile(manifestPath, artifact, label) {
  const resolved = await resolveManifestArtifactPath(manifestPath, artifact, label);
  const actualHash = await fileSha256(resolved);
  if (actualHash !== artifact.sha256) {
    throw new Error(`${label} sha256 must match material manifest.`);
  }
  return resolved;
}

function requireManifestValue(manifest, key, expected, label) {
  const value = ownValue(manifest, key);
  if (value !== expected) {
    throw new Error(`material manifest ${label} must be ${expected}.`);
  }
}

function requireManifestHash(manifest, key, expected, label) {
  const value = normalizeManifestHash(
    ownValue(manifest, key),
    `material manifest ${label}`,
  );
  if (value !== expected) {
    throw new Error(`material manifest ${label} must be ${expected}.`);
  }
}

function requireSnarkjsSelfCheck(selfCheck, key, expected, label) {
  const value = ownValue(selfCheck, key);
  if (value !== expected) {
    throw new Error(`material manifest SnarkJS self-check ${label} must be ${expected}.`);
  }
}

function validateMaterialManifestForAttestationRequest(manifest, profile) {
  if (!isRecord(manifest)) {
    throw new Error("material manifest must be a JSON object.");
  }
  const referenceBlockers = materialManifestReferenceBlockers(manifest);
  if (referenceBlockers.length > 0) {
    throw new Error(
      `material manifest references are not production-ready: ${referenceBlockers.join("; ")}`,
    );
  }
  requireManifestValue(
    manifest,
    "schema",
    BSC_GROTH16_MATERIAL_MANIFEST_SCHEMA,
    "schema",
  );
  requireManifestValue(manifest, "routeId", ROUTE_ID, "routeId");
  requireManifestValue(manifest, "assetKey", ASSET_KEY, "assetKey");
  requireManifestValue(manifest, "bscNetwork", profile.key, "bscNetwork");
  requireManifestValue(manifest, "chain", profile.chain, "chain");
  requireManifestValue(manifest, "chainIdHex", profile.chainIdHex, "chainIdHex");
  requireManifestHash(manifest, "networkIdHex", profile.networkIdHex, "networkIdHex");
  requireManifestValue(
    manifest,
    "circuitProfile",
    BSC_FULL_SCCP_CIRCUIT_PROFILE,
    "circuitProfile",
  );
  requireManifestValue(manifest, "publicInputCount", 9, "publicInputCount");
  const publicSignalNames = ownValue(manifest, "publicSignalNames");
  if (
    !Array.isArray(publicSignalNames) ||
    JSON.stringify(publicSignalNames) !==
      JSON.stringify(BSC_GROTH16_PUBLIC_SIGNAL_NAMES)
  ) {
    throw new Error(
      "material manifest publicSignalNames must match BSC Groth16 public signals.",
    );
  }
  normalizeManifestHash(
    ownValue(manifest, "verifierKeyHash"),
    "material manifest verifierKeyHash",
  );
  const selfChecks = ownValue(manifest, "selfChecks");
  const snarkjsSelfCheck = isRecord(selfChecks)
    ? ownValue(selfChecks, "snarkjs")
    : null;
  if (!isRecord(snarkjsSelfCheck)) {
    throw new Error("material manifest SnarkJS self-check is required.");
  }
  requireSnarkjsSelfCheck(snarkjsSelfCheck, "r1csInfo", true, "r1csInfo");
  if (trim(ownValue(snarkjsSelfCheck, "r1csInfoSource")) === "") {
    throw new Error(
      "material manifest SnarkJS self-check r1csInfoSource is required.",
    );
  }
  requireSnarkjsSelfCheck(
    snarkjsSelfCheck,
    "r1csPublicInputCount",
    9,
    "r1csPublicInputCount",
  );
  const constraintCount = Number(ownValue(snarkjsSelfCheck, "r1csConstraintCount"));
  if (
    !Number.isSafeInteger(constraintCount) ||
    constraintCount < PRODUCTION_FULL_SCCP_MIN_R1CS_CONSTRAINTS
  ) {
    throw new Error(
      `material manifest SnarkJS self-check r1csConstraintCount must be at least ${PRODUCTION_FULL_SCCP_MIN_R1CS_CONSTRAINTS}.`,
    );
  }
  requireSnarkjsSelfCheck(
    snarkjsSelfCheck,
    "zkeyVerify",
    true,
    "zkeyVerify",
  );
  if (trim(ownValue(snarkjsSelfCheck, "zkeyVerifyResult")) !== SNARKJS_ZKEY_VERIFY_OK) {
    throw new Error(
      `material manifest SnarkJS self-check zkeyVerifyResult must be ${SNARKJS_ZKEY_VERIFY_OK}.`,
    );
  }
  requireSnarkjsSelfCheck(
    snarkjsSelfCheck,
    "zkeyVerificationKeyExport",
    true,
    "zkeyVerificationKeyExport",
  );
  requireSnarkjsSelfCheck(
    snarkjsSelfCheck,
    "verifierKeyHashMatches",
    true,
    "verifierKeyHashMatches",
  );
  const verifierKeyHash = normalizeManifestHash(
    ownValue(manifest, "verifierKeyHash"),
    "material manifest verifierKeyHash",
  );
  const exportedVerifierKeyHash = normalizeManifestHash(
    ownValue(snarkjsSelfCheck, "exportedVerifierKeyHash"),
    "material manifest SnarkJS self-check exportedVerifierKeyHash",
  );
  if (exportedVerifierKeyHash !== verifierKeyHash) {
    throw new Error(
      "material manifest SnarkJS self-check exportedVerifierKeyHash must match verifierKeyHash.",
    );
  }
  const circuitSourceCheck = isRecord(selfChecks)
    ? ownValue(selfChecks, "circuitSource")
    : null;
  if (!isRecord(circuitSourceCheck)) {
    throw new Error("material manifest circuit-source self-check is required.");
  }
  requireSnarkjsSelfCheck(
    circuitSourceCheck,
    "fullMessageCircuit",
    true,
    "fullMessageCircuit",
  );
  requireSnarkjsSelfCheck(
    circuitSourceCheck,
    "signalBindingFixture",
    false,
    "signalBindingFixture",
  );
  requireSnarkjsSelfCheck(
    circuitSourceCheck,
    "unresolvedPlaceholders",
    false,
    "unresolvedPlaceholders",
  );
  requireSnarkjsSelfCheck(
    circuitSourceCheck,
    "keccakPublicSignalDerivation",
    true,
    "keccakPublicSignalDerivation",
  );
  requireSnarkjsSelfCheck(
    circuitSourceCheck,
    "digestReductionModuloScalarField",
    true,
    "digestReductionModuloScalarField",
  );
  requireSnarkjsSelfCheck(
    circuitSourceCheck,
    "valueBitBooleanConstraints",
    true,
    "valueBitBooleanConstraints",
  );
  requireSnarkjsSelfCheck(
    circuitSourceCheck,
    "publicSignalConstraintCount",
    9,
    "publicSignalConstraintCount",
  );
  requireSnarkjsSelfCheck(
    circuitSourceCheck,
    "labelBindingCount",
    9,
    "labelBindingCount",
  );
}

function proofSelfTestManifestProductionState(manifest) {
  const productionReady = ownValue(manifest, "productionReady") === true;
  const productionBlockers = ownValue(manifest, "productionBlockers");
  if (!Array.isArray(productionBlockers)) {
    throw new Error(
      "proof-self-test requires material manifest productionBlockers to be an empty array.",
    );
  }
  return {
    productionReady,
    productionBlockers: productionBlockers.map((blocker) => String(blocker)),
  };
}

function requireProductionReadyMaterialManifestForProofSelfTest(manifest) {
  const { productionReady, productionBlockers } =
    proofSelfTestManifestProductionState(manifest);
  if (productionReady !== true) {
    throw new Error(
      "proof-self-test requires a productionReady Groth16 material manifest.",
    );
  }
  if (productionBlockers.length > 0) {
    throw new Error(
      `proof-self-test requires a blocker-free Groth16 material manifest: ${productionBlockers
        .map((blocker) => String(blocker))
        .filter(Boolean)
        .join("; ")}`,
    );
  }
}

function attestationRequestCommonBody(manifest, artifacts) {
  return {
    routeId: ROUTE_ID,
    assetKey: ASSET_KEY,
    bscNetwork: ownValue(manifest, "bscNetwork"),
    chain: ownValue(manifest, "chain"),
    chainIdHex: ownValue(manifest, "chainIdHex"),
    networkIdHex: normalizeManifestHash(
      ownValue(manifest, "networkIdHex"),
      "material manifest networkIdHex",
    ),
    proofBackend: BSC_EVM_GROTH16_BACKEND,
    proofFamily: SCCP_PROOF_FAMILY_STARK_FRI,
    circuitProfile: BSC_FULL_SCCP_CIRCUIT_PROFILE,
    publicInputCount: 9,
    publicSignalNames: [...BSC_GROTH16_PUBLIC_SIGNAL_NAMES],
    verifierKeyHash: normalizeManifestHash(
      ownValue(manifest, "verifierKeyHash"),
      "material manifest verifierKeyHash",
    ),
    circuitSourceSha256: artifacts.circuitSource.sha256,
    r1csSha256: artifacts.r1cs.sha256,
    powersOfTauSha256: artifacts.powersOfTau.sha256,
    provingKeySha256: artifacts.provingKey.sha256,
    snarkjsVerificationKeySha256: artifacts.snarkjsVerificationKey.sha256,
    bscVerifierKeySha256: artifacts.bscVerifierKey.sha256,
  };
}

function defaultWitnessWasmPathFromR1cs(r1csPath) {
  const stem = basename(r1csPath, ".r1cs");
  return join(dirname(r1csPath), `${stem}_js`, `${stem}.wasm`);
}

function publicSignalMismatch(expected, observed) {
  for (const [index, expectedWord] of expected.entries()) {
    if (observed[index] !== expectedWord) {
      return `public signal ${index} (${BSC_GROTH16_PUBLIC_SIGNAL_NAMES[index]}) expected ${expectedWord} but got ${observed[index]}`;
    }
  }
  return "";
}

function requireProofSelfTestHash(record, keys, label) {
  normalizeManifestHash(attestationValue(record, keys), label);
}

function requireProofSelfTestValue(report, key, expected, label) {
  const value = ownValue(report, key);
  if (value !== expected) {
    throw new Error(`${label} must be ${expected}.`);
  }
}

function validateProofSelfTestAdversarialChecks(report, blockers) {
  const checks = isRecord(ownValue(report, "adversarialChecks"))
    ? ownValue(report, "adversarialChecks")
    : null;
  if (!checks) {
    blockers.push("proof self-test adversarialChecks block is required");
    return;
  }
  const publicSignalMismatch = isRecord(ownValue(checks, "publicSignalMismatch"))
    ? ownValue(checks, "publicSignalMismatch")
    : null;
  if (!publicSignalMismatch) {
    blockers.push("proof self-test adversarialChecks.publicSignalMismatch is required");
  } else {
    if (ownValue(publicSignalMismatch, "attempted") !== 9) {
      blockers.push("proof self-test adversarial publicSignalMismatch.attempted must be 9");
    }
    if (ownValue(publicSignalMismatch, "rejected") !== 9) {
      blockers.push("proof self-test adversarial publicSignalMismatch.rejected must be 9");
    }
    const cases = ownValue(publicSignalMismatch, "cases");
    if (!Array.isArray(cases) || cases.length !== 9) {
      blockers.push("proof self-test adversarial publicSignalMismatch.cases must contain 9 entries");
    } else {
      for (const [index, entry] of cases.entries()) {
        if (!isRecord(entry)) {
          blockers.push(
            `proof self-test adversarial publicSignalMismatch.cases[${index}] must be an object`,
          );
          continue;
        }
        if (ownValue(entry, "index") !== index) {
          blockers.push(
            `proof self-test adversarial publicSignalMismatch.cases[${index}].index must be ${index}`,
          );
        }
        if (ownValue(entry, "name") !== BSC_GROTH16_PUBLIC_SIGNAL_NAMES[index]) {
          blockers.push(
            `proof self-test adversarial publicSignalMismatch.cases[${index}].name must be ${BSC_GROTH16_PUBLIC_SIGNAL_NAMES[index]}`,
          );
        }
        if (ownValue(entry, "rejected") !== true) {
          blockers.push(
            `proof self-test adversarial publicSignalMismatch.cases[${index}].rejected must be true`,
          );
        }
        if (ownValue(entry, "phase") !== "wtnsCalculate") {
          blockers.push(
            `proof self-test adversarial publicSignalMismatch.cases[${index}].phase must be wtnsCalculate`,
          );
        }
      }
    }
  }

  const nonBooleanValueBit = isRecord(ownValue(checks, "nonBooleanValueBit"))
    ? ownValue(checks, "nonBooleanValueBit")
    : null;
  if (!nonBooleanValueBit) {
    blockers.push("proof self-test adversarialChecks.nonBooleanValueBit is required");
  } else {
    if (ownValue(nonBooleanValueBit, "attempted") !== 1) {
      blockers.push("proof self-test adversarial nonBooleanValueBit.attempted must be 1");
    }
    if (ownValue(nonBooleanValueBit, "rejected") !== 1) {
      blockers.push("proof self-test adversarial nonBooleanValueBit.rejected must be 1");
    }
    const testCase = isRecord(ownValue(nonBooleanValueBit, "case"))
      ? ownValue(nonBooleanValueBit, "case")
      : null;
    if (!testCase) {
      blockers.push("proof self-test adversarial nonBooleanValueBit.case is required");
    } else {
      if (ownValue(testCase, "signalName") !== BSC_GROTH16_PUBLIC_SIGNAL_NAMES[0]) {
        blockers.push(
          `proof self-test adversarial nonBooleanValueBit.case.signalName must be ${BSC_GROTH16_PUBLIC_SIGNAL_NAMES[0]}`,
        );
      }
      if (ownValue(testCase, "inputName") !== BSC_GROTH16_SIGNAL_INPUT_NAMES[0]) {
        blockers.push(
          `proof self-test adversarial nonBooleanValueBit.case.inputName must be ${BSC_GROTH16_SIGNAL_INPUT_NAMES[0]}`,
        );
      }
      if (ownValue(testCase, "bitIndex") !== 0) {
        blockers.push("proof self-test adversarial nonBooleanValueBit.case.bitIndex must be 0");
      }
      if (ownValue(testCase, "rejected") !== true) {
        blockers.push("proof self-test adversarial nonBooleanValueBit.case.rejected must be true");
      }
      if (ownValue(testCase, "phase") !== "wtnsCalculate") {
        blockers.push("proof self-test adversarial nonBooleanValueBit.case.phase must be wtnsCalculate");
      }
    }
  }
}

function unknownProofSelfTestFields(record, allowedFields, label) {
  if (!isRecord(record)) {
    return [];
  }
  return Object.keys(record)
    .filter((key) => !allowedFields.has(key))
    .map((key) => `${label} contains unknown field: ${key}`);
}

function proofSelfTestShapeBlockers(report) {
  const blockers = [
    ...unknownProofSelfTestFields(
      report,
      new Set([
        "schema",
        "routeId",
        "assetKey",
        "bscNetwork",
        "chain",
        "chainIdHex",
        "networkIdHex",
        "circuitProfile",
        "proofBackend",
        "proofFamily",
        "generatedAt",
        "manifest",
        "artifacts",
        "sample",
        "witnessHash",
        "proofHash",
        "publicSignalsHash",
        "snarkjs",
        "adversarialChecks",
        "proof",
        "publicSignals",
      ]),
      "proof self-test report",
    ),
  ];
  const manifest = ownValue(report, "manifest");
  blockers.push(
    ...unknownProofSelfTestFields(
      manifest,
      new Set(["path", "sha256", "productionReady", "productionBlockers"]),
      "proof self-test manifest",
    ),
  );
  const artifacts = ownValue(report, "artifacts");
  blockers.push(
    ...unknownProofSelfTestFields(
      artifacts,
      new Set([
        "circuitSource",
        "r1cs",
        "provingKey",
        "snarkjsVerificationKey",
        "bscVerifierKey",
        "witnessWasm",
      ]),
      "proof self-test artifacts",
    ),
  );
  if (isRecord(artifacts)) {
    for (const [key, label] of [
      ["circuitSource", "circuit source artifact"],
      ["r1cs", "R1CS artifact"],
      ["provingKey", "proving key artifact"],
      ["snarkjsVerificationKey", "SnarkJS verification key artifact"],
      ["bscVerifierKey", "BSC verifier key artifact"],
      ["witnessWasm", "witness WASM artifact"],
    ]) {
      blockers.push(
        ...unknownProofSelfTestFields(
          ownValue(artifacts, key),
          new Set(["path", "sha256"]),
          `proof self-test ${label}`,
        ),
      );
    }
  }
  const sample = ownValue(report, "sample");
  blockers.push(
    ...unknownProofSelfTestFields(
      sample,
      new Set([
        "id",
        "syntheticInputWords",
        "publicSignalNames",
        "publicSignalWords",
        "inputSha256",
      ]),
      "proof self-test sample",
    ),
  );
  const syntheticInputWords = isRecord(sample)
    ? ownValue(sample, "syntheticInputWords")
    : null;
  blockers.push(
    ...unknownProofSelfTestFields(
      syntheticInputWords,
      new Set(BSC_GROTH16_PUBLIC_SIGNAL_NAMES),
      "proof self-test sample.syntheticInputWords",
    ),
  );
  const snarkjs = ownValue(report, "snarkjs");
  blockers.push(
    ...unknownProofSelfTestFields(
      snarkjs,
      new Set(["binary", "wtnsCalculate", "groth16Prove", "groth16Verify"]),
      "proof self-test snarkjs",
    ),
  );
  const adversarialChecks = ownValue(report, "adversarialChecks");
  blockers.push(
    ...unknownProofSelfTestFields(
      adversarialChecks,
      new Set(["publicSignalMismatch", "nonBooleanValueBit"]),
      "proof self-test adversarialChecks",
    ),
  );
  const publicSignalMismatch = isRecord(adversarialChecks)
    ? ownValue(adversarialChecks, "publicSignalMismatch")
    : null;
  blockers.push(
    ...unknownProofSelfTestFields(
      publicSignalMismatch,
      new Set(["attempted", "rejected", "cases"]),
      "proof self-test adversarialChecks.publicSignalMismatch",
    ),
  );
  const publicSignalCases = isRecord(publicSignalMismatch)
    ? ownValue(publicSignalMismatch, "cases")
    : null;
  if (Array.isArray(publicSignalCases)) {
    for (const [index, entry] of publicSignalCases.entries()) {
      blockers.push(
        ...unknownProofSelfTestFields(
          entry,
          new Set(["index", "name", "phase", "rejected"]),
          `proof self-test adversarialChecks.publicSignalMismatch.cases[${index}]`,
        ),
      );
    }
  }
  const nonBooleanValueBit = isRecord(adversarialChecks)
    ? ownValue(adversarialChecks, "nonBooleanValueBit")
    : null;
  blockers.push(
    ...unknownProofSelfTestFields(
      nonBooleanValueBit,
      new Set(["attempted", "rejected", "case"]),
      "proof self-test adversarialChecks.nonBooleanValueBit",
    ),
  );
  const nonBooleanCase = isRecord(nonBooleanValueBit)
    ? ownValue(nonBooleanValueBit, "case")
    : null;
  blockers.push(
    ...unknownProofSelfTestFields(
      nonBooleanCase,
      new Set(["signalName", "inputName", "bitIndex", "phase", "rejected"]),
      "proof self-test adversarialChecks.nonBooleanValueBit.case",
    ),
  );
  blockers.push(
    ...unknownProofSelfTestFields(
      ownValue(report, "proof"),
      new Set(["pi_a", "pi_b", "pi_c", "protocol", "curve"]),
      "proof self-test proof",
    ),
  );
  return blockers;
}

function proofSelfTestDecimalWordBlockers(values, expectedLength, label) {
  if (!Array.isArray(values) || values.length !== expectedLength) {
    return [`${label} must contain ${expectedLength} canonical decimal BN254 field words`];
  }
  return values.flatMap((entry, index) => {
    try {
      canonicalDecimalFieldWord(entry, `${label}[${index}]`);
      return [];
    } catch (error) {
      return [error instanceof Error ? error.message : String(error)];
    }
  });
}

function proofSelfTestProofBlockers(proof) {
  if (!isRecord(proof)) {
    return ["proof self-test proof object is required"];
  }
  const blockers = [];
  if (ownValue(proof, "protocol") !== "groth16") {
    blockers.push("proof self-test proof.protocol must be groth16");
  }
  if (ownValue(proof, "curve") !== "bn128") {
    blockers.push("proof self-test proof.curve must be bn128");
  }
  blockers.push(
    ...proofSelfTestDecimalWordBlockers(
      ownValue(proof, "pi_a"),
      3,
      "proof self-test proof.pi_a",
    ),
  );
  const piB = ownValue(proof, "pi_b");
  if (!Array.isArray(piB) || piB.length !== 3) {
    blockers.push("proof self-test proof.pi_b must contain 3 coordinate pairs");
  } else {
    for (const [index, row] of piB.entries()) {
      blockers.push(
        ...proofSelfTestDecimalWordBlockers(
          row,
          2,
          `proof self-test proof.pi_b[${index}]`,
        ),
      );
    }
  }
  blockers.push(
    ...proofSelfTestDecimalWordBlockers(
      ownValue(proof, "pi_c"),
      3,
      "proof self-test proof.pi_c",
    ),
  );
  return blockers;
}

async function validateProofSelfTestReport({
  reportPath,
  profile,
  circuitProfile,
  manifestPath,
  paths,
}) {
  const blockers = [];
  let report;
  try {
    report = await readJson(reportPath, "BSC Groth16 proof self-test report");
  } catch (error) {
    return [error instanceof Error ? error.message : String(error)];
  }
  const secretReason = unsafeSecretReason(report, "BSC Groth16 proof self-test report");
  if (secretReason) {
    return [secretReason];
  }
  blockers.push(...proofSelfTestShapeBlockers(report));
  const check = (fn) => {
    try {
      fn();
    } catch (error) {
      blockers.push(error instanceof Error ? error.message : String(error));
    }
  };
  check(() => requireProofSelfTestValue(
    report,
    "schema",
    BSC_GROTH16_PROOF_SELF_TEST_SCHEMA,
    "proof self-test schema",
  ));
  check(() => requireProofSelfTestValue(report, "routeId", ROUTE_ID, "proof self-test routeId"));
  check(() => requireProofSelfTestValue(report, "assetKey", ASSET_KEY, "proof self-test assetKey"));
  check(() => requireProofSelfTestValue(
    report,
    "bscNetwork",
    profile.key,
    "proof self-test bscNetwork",
  ));
  check(() => requireProofSelfTestValue(
    report,
    "chain",
    profile.chain,
    "proof self-test chain",
  ));
  check(() => requireProofSelfTestValue(
    report,
    "chainIdHex",
    profile.chainIdHex,
    "proof self-test chainIdHex",
  ));
  check(() => {
    const networkIdHex = normalizeManifestHash(
      ownValue(report, "networkIdHex"),
      "proof self-test networkIdHex",
    );
    if (networkIdHex !== profile.networkIdHex) {
      throw new Error(
        `proof self-test networkIdHex must be ${profile.networkIdHex}.`,
      );
    }
  });
  check(() => requireProofSelfTestValue(
    report,
    "circuitProfile",
    circuitProfile,
    "proof self-test circuitProfile",
  ));
  check(() => requireProofSelfTestValue(
    report,
    "proofBackend",
    BSC_EVM_GROTH16_BACKEND,
    "proof self-test proofBackend",
  ));
  check(() => requireProofSelfTestValue(
    report,
    "proofFamily",
    SCCP_PROOF_FAMILY_STARK_FRI,
    "proof self-test proofFamily",
  ));
  const manifestBlock = isRecord(ownValue(report, "manifest"))
    ? ownValue(report, "manifest")
    : null;
  if (!manifestBlock) {
    blockers.push("proof self-test manifest block is required");
  } else {
    const expectedManifestSha256 = await fileSha256(manifestPath);
    const blocker = hashEqualsBlocker(
      manifestBlock,
      ["sha256"],
      expectedManifestSha256,
      "proof self-test manifest.sha256",
    );
    if (blocker) blockers.push(blocker);
    if (ownValue(manifestBlock, "productionReady") !== true) {
      blockers.push("proof self-test manifest.productionReady must be true");
    }
    const productionBlockers = ownValue(manifestBlock, "productionBlockers");
    if (!Array.isArray(productionBlockers)) {
      blockers.push(
        "proof self-test manifest.productionBlockers must be an empty array",
      );
    } else if (productionBlockers.length > 0) {
      blockers.push(
        `proof self-test manifest.productionBlockers must be empty: ${productionBlockers
          .map((entry) => String(entry))
          .filter(Boolean)
          .join("; ")}`,
      );
    }
  }
  const artifactBlock = isRecord(ownValue(report, "artifacts"))
    ? ownValue(report, "artifacts")
    : null;
  if (!artifactBlock) {
    blockers.push("proof self-test artifacts block is required");
  } else {
    for (const [key, label] of [
      ["circuitSource", "circuit source"],
      ["r1cs", "R1CS"],
      ["provingKey", "proving key"],
      ["snarkjsVerificationKey", "SnarkJS verification key"],
      ["bscVerifierKey", "BSC verifier key"],
      ["witnessWasm", "witness WASM"],
    ]) {
      const artifact = isRecord(ownValue(artifactBlock, key))
        ? ownValue(artifactBlock, key)
        : null;
      if (!artifact) {
        blockers.push(`proof self-test ${key} artifact is required`);
        continue;
      }
      const expectedHash = await fileSha256(paths[key]);
      const blocker = hashEqualsBlocker(
        artifact,
        ["sha256"],
        expectedHash,
        `proof self-test ${label} sha256`,
      );
      if (blocker) blockers.push(blocker);
    }
  }
  const sample = isRecord(ownValue(report, "sample")) ? ownValue(report, "sample") : null;
  if (!sample) {
    blockers.push("proof self-test sample block is required");
  } else {
    const publicSignalNames = ownValue(sample, "publicSignalNames");
    if (
      !Array.isArray(publicSignalNames) ||
      JSON.stringify(publicSignalNames) !==
        JSON.stringify(BSC_GROTH16_PUBLIC_SIGNAL_NAMES)
    ) {
      blockers.push(
        "proof self-test publicSignalNames must match BSC Groth16 public signals",
      );
    }
    let expectedSignals = null;
    try {
      expectedSignals = normalizeSnarkjsPublicSignals(
        ownValue(sample, "publicSignalWords"),
      );
    } catch (error) {
      blockers.push(error instanceof Error ? error.message : String(error));
    }
    let normalizedPublicSignals = null;
    try {
      normalizedPublicSignals = normalizeSnarkjsPublicSignals(
        ownValue(report, "publicSignals"),
      );
      if (expectedSignals) {
        const mismatch = publicSignalMismatch(
          expectedSignals,
          normalizedPublicSignals,
        );
        if (mismatch) {
          blockers.push(`proof self-test public signals mismatch: ${mismatch}`);
        }
      }
    } catch (error) {
      blockers.push(error instanceof Error ? error.message : String(error));
    }
    if (normalizedPublicSignals) {
      try {
        const publicSignalsHash = normalizeManifestHash(
          ownValue(report, "publicSignalsHash") ??
            ownValue(report, "public_signals_hash"),
          "proof self-test publicSignalsHash",
        );
        const actualPublicSignalsHash = sha256Hex(
          Buffer.from(canonicalJson(normalizedPublicSignals), "utf8"),
        );
        if (publicSignalsHash !== actualPublicSignalsHash) {
          blockers.push(
            "proof self-test publicSignalsHash must match publicSignals",
          );
        }
      } catch (error) {
        blockers.push(error instanceof Error ? error.message : String(error));
      }
    }
  }
  const snarkjs = isRecord(ownValue(report, "snarkjs")) ? ownValue(report, "snarkjs") : null;
  if (!snarkjs) {
    blockers.push("proof self-test snarkjs block is required");
  } else {
    for (const key of ["wtnsCalculate", "groth16Prove", "groth16Verify"]) {
      if (ownValue(snarkjs, key) !== true) {
        blockers.push(`proof self-test snarkjs.${key} must be true`);
      }
    }
  }
  for (const [keys, label] of [
    [["witnessHash", "witness_hash"], "proof self-test witnessHash"],
    [["proofHash", "proof_hash"], "proof self-test proofHash"],
    [["publicSignalsHash", "public_signals_hash"], "proof self-test publicSignalsHash"],
  ]) {
    check(() => requireProofSelfTestHash(report, keys, label));
  }
  const proof = isRecord(ownValue(report, "proof")) ? ownValue(report, "proof") : null;
  if (!proof) {
    blockers.push("proof self-test proof object is required");
  } else {
    blockers.push(...proofSelfTestProofBlockers(proof));
    try {
      const proofHash = normalizeManifestHash(
        ownValue(report, "proofHash") ?? ownValue(report, "proof_hash"),
        "proof self-test proofHash",
      );
      const actualProofHash = sha256Hex(
        Buffer.from(canonicalJson(proof), "utf8"),
      );
      if (proofHash !== actualProofHash) {
        blockers.push("proof self-test proofHash must match proof");
      }
    } catch (error) {
      blockers.push(error instanceof Error ? error.message : String(error));
    }
  }
  validateProofSelfTestAdversarialChecks(report, blockers);
  return blockers.filter(Boolean);
}

async function expectWitnessCalculationRejection({
  snarkjsBin,
  witnessWasm,
  inputPath,
  witnessPath,
  label,
}) {
  try {
    await runCommand(snarkjsBin, [
      "wtns",
      "calculate",
      witnessWasm,
      inputPath,
      witnessPath,
    ]);
  } catch (_error) {
    return true;
  }
  throw new Error(`SnarkJS proof self-test adversarial ${label} was accepted`);
}

async function runBscGroth16AdversarialChecks({
  snarkjsBin,
  witnessWasm,
  tempRoot,
  sample,
}) {
  const cases = [];
  for (const [index, signalName] of BSC_GROTH16_PUBLIC_SIGNAL_NAMES.entries()) {
    const input = cloneJson(sample.input);
    input.publicSignals[index] = nextDecimalFieldWord(input.publicSignals[index]);
    const inputPath = join(tempRoot, `adversarial-public-signal-${index}.json`);
    const witnessPath = join(tempRoot, `adversarial-public-signal-${index}.wtns`);
    await writePublicJson(inputPath, input);
    await expectWitnessCalculationRejection({
      snarkjsBin,
      witnessWasm,
      inputPath,
      witnessPath,
      label: `publicSignalMismatch.${signalName}`,
    });
    cases.push({
      index,
      name: signalName,
      phase: "wtnsCalculate",
      rejected: true,
    });
  }

  const nonBooleanInput = cloneJson(sample.input);
  nonBooleanInput[BSC_GROTH16_SIGNAL_INPUT_NAMES[0]][0] = 2;
  const nonBooleanInputPath = join(tempRoot, "adversarial-non-boolean-bit.json");
  const nonBooleanWitnessPath = join(tempRoot, "adversarial-non-boolean-bit.wtns");
  await writePublicJson(nonBooleanInputPath, nonBooleanInput);
  await expectWitnessCalculationRejection({
    snarkjsBin,
    witnessWasm,
    inputPath: nonBooleanInputPath,
    witnessPath: nonBooleanWitnessPath,
    label: "nonBooleanValueBit.message_id[0]",
  });

  return {
    publicSignalMismatch: {
      attempted: cases.length,
      rejected: cases.length,
      cases,
    },
    nonBooleanValueBit: {
      attempted: 1,
      rejected: 1,
      case: {
        signalName: BSC_GROTH16_PUBLIC_SIGNAL_NAMES[0],
        inputName: BSC_GROTH16_SIGNAL_INPUT_NAMES[0],
        bitIndex: 0,
        phase: "wtnsCalculate",
        rejected: true,
      },
    },
  };
}

export async function runBscGroth16ProofSelfTest(options = {}) {
  const manifestPath = await assertReadableRegularFile(
    requiredOption(
      options,
      ["manifest", "material-manifest", "groth16-material-manifest"],
      "BSC Groth16 proof self-test",
    ),
    "BSC Groth16 material manifest",
  );
  const manifest = await readJson(manifestPath, "BSC Groth16 material manifest");
  const secretReason = unsafeSecretReason(manifest, "BSC Groth16 material manifest");
  if (secretReason) {
    throw new Error(secretReason);
  }
  const profile = normalizeBscNetworkProfile(
    ownValue(options, "bsc-network") ??
      ownValue(options, "network") ??
      ownValue(manifest, "bscNetwork"),
  );
  validateMaterialManifestForAttestationRequest(manifest, profile);
  const allowUnreadyCandidate = optionEnabled(
    options,
    "allow-unready-candidate",
    false,
  );
  const allowUnreadyMainnetCandidate = optionEnabled(
    options,
    "allow-unready-mainnet-candidate",
    false,
  );
  const manifestProductionState = proofSelfTestManifestProductionState(manifest);
  if (
    manifestProductionState.productionReady !== true ||
    manifestProductionState.productionBlockers.length > 0
  ) {
    if (!allowUnreadyCandidate && !allowUnreadyMainnetCandidate) {
      requireProductionReadyMaterialManifestForProofSelfTest(manifest);
    }
    if (allowUnreadyCandidate && profile.key !== "testnet") {
      throw new Error(
        "--allow-unready-candidate is only allowed for testnet candidate proof reports.",
      );
    }
    if (allowUnreadyMainnetCandidate && profile.key !== "mainnet") {
      throw new Error(
        "--allow-unready-mainnet-candidate is only allowed for mainnet candidate proof reports.",
      );
    }
    if (profile.key === "testnet" && !allowUnreadyCandidate) {
      throw new Error(
        "--allow-unready-candidate true is required to refresh unready testnet candidate proof reports.",
      );
    }
    if (profile.key === "mainnet" && !allowUnreadyMainnetCandidate) {
      throw new Error(
        "--allow-unready-mainnet-candidate true is required to refresh unready mainnet candidate proof reports.",
      );
    }
  }
  const artifacts = {
    circuitSource: materialManifestArtifact(manifest, "circuitSource", "circuitSource"),
    r1cs: materialManifestArtifact(manifest, "r1cs", "r1cs"),
    provingKey: materialManifestArtifact(manifest, "provingKey", "provingKey"),
    snarkjsVerificationKey: materialManifestArtifact(
      manifest,
      "snarkjsVerificationKey",
      "snarkjsVerificationKey",
    ),
    bscVerifierKey: materialManifestArtifact(
      manifest,
      "bscVerifierKey",
      "bscVerifierKey",
    ),
  };
  const resolvedArtifacts = {
    circuitSource: await resolveManifestArtifactFile(
      manifestPath,
      artifacts.circuitSource,
      "circuit source",
    ),
    r1cs: await resolveManifestArtifactFile(manifestPath, artifacts.r1cs, "R1CS"),
    provingKey: await resolveManifestArtifactFile(
      manifestPath,
      artifacts.provingKey,
      "proving key",
    ),
    snarkjsVerificationKey: await resolveManifestArtifactFile(
      manifestPath,
      artifacts.snarkjsVerificationKey,
      "SnarkJS verification key",
    ),
    bscVerifierKey: await resolveManifestArtifactFile(
      manifestPath,
      artifacts.bscVerifierKey,
      "BSC verifier key",
    ),
  };
  const witnessWasm = await assertReadableRegularFile(
    optionalPath(options, ["witness-wasm", "wasm"]) ??
      defaultWitnessWasmPathFromR1cs(resolvedArtifacts.r1cs),
    "witness WASM",
  );
  const snarkjsBin = commandValue(options, "snarkjs-bin", "snarkjs");
  const sample = bscGroth16SelfTestInput(profile);
  const tempRoot = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-proof-self-test-"));
  try {
    const inputPath = join(tempRoot, "input.json");
    const witnessPath = join(tempRoot, "witness.wtns");
    const proofPath = join(tempRoot, "proof.json");
    const publicPath = join(tempRoot, "public.json");
    await writePublicJson(inputPath, sample.input);
    await runCommand(snarkjsBin, [
      "wtns",
      "calculate",
      witnessWasm,
      inputPath,
      witnessPath,
    ]);
    await runCommand(snarkjsBin, [
      "groth16",
      "prove",
      resolvedArtifacts.provingKey,
      witnessPath,
      proofPath,
      publicPath,
    ]);
    const proof = await readJson(proofPath, "SnarkJS proof");
    const publicSignals = normalizeSnarkjsPublicSignals(
      await readJson(publicPath, "SnarkJS public signals"),
    );
    const mismatch = publicSignalMismatch(sample.publicSignalWords, publicSignals);
    if (mismatch) {
      throw new Error(`SnarkJS proof self-test public signals mismatch: ${mismatch}`);
    }
    await runCommand(snarkjsBin, [
      "groth16",
      "verify",
      resolvedArtifacts.snarkjsVerificationKey,
      publicPath,
      proofPath,
    ]);
    const adversarialChecks = await runBscGroth16AdversarialChecks({
      snarkjsBin,
      witnessWasm,
      tempRoot,
      sample,
    });
    const report = {
      schema: BSC_GROTH16_PROOF_SELF_TEST_SCHEMA,
      routeId: ROUTE_ID,
      assetKey: ASSET_KEY,
      bscNetwork: profile.key,
      chain: profile.chain,
      chainIdHex: profile.chainIdHex,
      networkIdHex: profile.networkIdHex,
      circuitProfile: BSC_FULL_SCCP_CIRCUIT_PROFILE,
      proofBackend: ownValue(manifest, "proofBackend") ?? BSC_EVM_GROTH16_BACKEND,
      proofFamily: ownValue(manifest, "proofFamily") ?? SCCP_PROOF_FAMILY_STARK_FRI,
      generatedAt: new Date().toISOString(),
      manifest: {
        path: repoRelativePath(manifestPath),
        sha256: await fileSha256(manifestPath),
        productionReady: manifestProductionState.productionReady,
        productionBlockers: manifestProductionState.productionBlockers,
      },
      artifacts: {
        circuitSource: artifacts.circuitSource,
        r1cs: artifacts.r1cs,
        provingKey: artifacts.provingKey,
        snarkjsVerificationKey: artifacts.snarkjsVerificationKey,
        bscVerifierKey: artifacts.bscVerifierKey,
        witnessWasm: {
          path: repoRelativePath(witnessWasm),
          sha256: await fileSha256(witnessWasm),
        },
      },
      sample: {
        id: sample.sampleId,
        syntheticInputWords: sample.syntheticInputWords,
        publicSignalNames: [...BSC_GROTH16_PUBLIC_SIGNAL_NAMES],
        publicSignalWords: sample.publicSignalWords,
        inputSha256: sha256Hex(Buffer.from(canonicalJson(sample.input), "utf8")),
      },
      witnessHash: await fileSha256(witnessPath),
      proofHash: sha256Hex(Buffer.from(canonicalJson(proof), "utf8")),
      publicSignalsHash: sha256Hex(
        Buffer.from(canonicalJson(publicSignals), "utf8"),
      ),
      snarkjs: {
        binary: displayCommandValue(snarkjsBin),
        wtnsCalculate: true,
        groth16Prove: true,
        groth16Verify: true,
      },
      adversarialChecks,
      proof,
      publicSignals,
    };
    const outPath =
      optionalPath(options, "out") ??
      join(dirname(manifestPath), `${profile.key}-bsc-groth16-proof-self-test.json`);
    await writePublicJson(outPath, report);
    return {
      ok: true,
      out: outPath,
      manifest: manifestPath,
      manifestSha256: report.manifest.sha256,
      proofHash: report.proofHash,
      publicSignalsHash: report.publicSignalsHash,
      witnessHash: report.witnessHash,
    };
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
}

function attestationRequestRole({
  signerRole,
  body,
  readyForSignature = true,
  blockers = [],
}) {
  const signedPayloadSha256 = sha256Hex(attestationSignaturePayload(body));
  return {
    signerRole,
    attestationSchema: body.schema,
    readyForSignature: Boolean(readyForSignature && blockers.length === 0),
    blockers: blockers.map((blocker) => String(blocker)),
    signedPayloadSha256,
    body,
    signatureTemplate: {
      schema: BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA,
      algorithm: "ed25519",
      signerFingerprint: "<sha256-of-ed25519-spki-public-key>",
      publicKeyPem: "<ed25519-spki-public-key-pem>",
      signedPayloadSha256,
      signature: "<base64-ed25519-signature-over-canonical-body-json>",
    },
  };
}

function normalizeOptionalToolchainHash(value) {
  if (value === undefined || value === null || trim(value) === "") {
    return null;
  }
  return normalizeManifestHash(value, "toolchainSha256");
}

function toolchainSha256FromTranscript(record, options) {
  const explicitHash = normalizeOptionalToolchainHash(
    ownValue(options, "toolchain-sha256") ?? ownValue(options, "toolchain-hash"),
  );
  if (explicitHash) {
    return explicitHash;
  }
  const toolchain = ownValue(record, "toolchain");
  if (!isRecord(toolchain)) {
    throw new Error(
      "reproducible build transcript toolchain object is required to derive toolchainSha256; pass --toolchain-sha256 only when an independent reproducible-build record supplies the hash out of band.",
    );
  }
  return sha256Hex(Buffer.from(canonicalJson(toolchain), "utf8"));
}

export async function generateBscGroth16AttestationRequestPackage(options = {}) {
  const manifestPath = await assertReadableRegularFile(
    requiredOption(
      options,
      ["manifest", "material-manifest", "groth16-material-manifest"],
      "attestation request package",
    ),
    "BSC Groth16 material manifest",
  );
  const manifest = await readJson(manifestPath, "BSC Groth16 material manifest");
  const secretReason = unsafeSecretReason(manifest, "BSC Groth16 material manifest");
  if (secretReason) {
    throw new Error(secretReason);
  }
  const profile = normalizeBscNetworkProfile(
    ownValue(options, "bsc-network") ??
      ownValue(options, "network") ??
      ownValue(manifest, "bscNetwork"),
  );
  validateMaterialManifestForAttestationRequest(manifest, profile);
  const artifacts = {
    circuitSource: materialManifestArtifact(manifest, "circuitSource", "circuitSource"),
    r1cs: materialManifestArtifact(manifest, "r1cs", "r1cs"),
    powersOfTau: materialManifestArtifact(manifest, "powersOfTau", "powersOfTau"),
    provingKey: materialManifestArtifact(manifest, "provingKey", "provingKey"),
    snarkjsVerificationKey: materialManifestArtifact(
      manifest,
      "snarkjsVerificationKey",
      "snarkjsVerificationKey",
    ),
    bscVerifierKey: materialManifestArtifact(
      manifest,
      "bscVerifierKey",
      "bscVerifierKey",
    ),
    trustedSetupTranscript: materialManifestArtifact(
      manifest,
      "trustedSetupTranscript",
      "trustedSetupTranscript",
    ),
    reproducibleBuildTranscript: materialManifestArtifact(
      manifest,
      "reproducibleBuildTranscript",
      "reproducibleBuildTranscript",
    ),
  };
  const trustedSetupTranscript = await readManifestJsonArtifact(
    manifestPath,
    artifacts.trustedSetupTranscript,
    "trusted setup transcript",
  );
  const reproducibleBuildTranscript = await readManifestJsonArtifact(
    manifestPath,
    artifacts.reproducibleBuildTranscript,
    "reproducible build transcript",
  );
  const selfChecks = ownValue(manifest, "selfChecks");
  const snarkjsSelfCheck = ownValue(selfChecks, "snarkjs");
  const trustedSetupTranscriptBlockers = await validateTrustedSetupTranscript(
    trustedSetupTranscript.path,
  );
  const reproducibleBuildTranscriptBlockers =
    await validateReproducibleBuildTranscript(
      reproducibleBuildTranscript.path,
      snarkjsSelfCheck,
    );
  const commonBody = attestationRequestCommonBody(manifest, artifacts);
  const setupBody = {
    schema: BSC_GROTH16_TRUSTED_SETUP_ATTESTATION_SCHEMA,
    ...commonBody,
    ceremonyResult: "pass",
    localSingleContributor: false,
    minimumContributors: 2,
    toxicWasteDestroyed: true,
    contributionTranscriptSha256: artifacts.trustedSetupTranscript.sha256,
  };
  const reproducibleBody = {
    schema: BSC_GROTH16_REPRODUCIBLE_BUILD_ATTESTATION_SCHEMA,
    ...commonBody,
    reproducible: true,
    independentRebuilders: 2,
    buildTranscriptSha256: artifacts.reproducibleBuildTranscript.sha256,
    toolchainSha256: toolchainSha256FromTranscript(
      reproducibleBuildTranscript.record,
      options,
    ),
    r1csInfoSource: ownValue(snarkjsSelfCheck, "r1csInfoSource"),
    r1csPublicInputCount: ownValue(snarkjsSelfCheck, "r1csPublicInputCount"),
    r1csConstraintCount: ownValue(snarkjsSelfCheck, "r1csConstraintCount"),
    zkeyVerify: ownValue(snarkjsSelfCheck, "zkeyVerify"),
    zkeyVerifyResult: ownValue(snarkjsSelfCheck, "zkeyVerifyResult"),
    zkeyVerificationKeyExport: ownValue(snarkjsSelfCheck, "zkeyVerificationKeyExport"),
    verifierKeyHashMatches: ownValue(snarkjsSelfCheck, "verifierKeyHashMatches"),
    exportedVerifierKeyHash: normalizeManifestHash(
      ownValue(snarkjsSelfCheck, "exportedVerifierKeyHash"),
      "material manifest SnarkJS self-check exportedVerifierKeyHash",
    ),
  };
  const productionBlockers = Array.isArray(ownValue(manifest, "productionBlockers"))
    ? ownValue(manifest, "productionBlockers").map((blocker) => String(blocker))
    : [];
  const packageBody = {
    schema: BSC_GROTH16_ATTESTATION_REQUEST_PACKAGE_SCHEMA,
    manifest: {
      path: repoRelativePath(manifestPath),
      sha256: await fileSha256(manifestPath),
      generatedAt: ownValue(manifest, "generatedAt") ?? null,
      productionReady: ownValue(manifest, "productionReady") === true,
      productionBlockers,
    },
    routeId: ROUTE_ID,
    assetKey: ASSET_KEY,
    bscNetwork: profile.key,
    chain: profile.chain,
    chainIdHex: profile.chainIdHex,
    networkIdHex: profile.networkIdHex,
    circuitProfile: BSC_FULL_SCCP_CIRCUIT_PROFILE,
    proofBackend: ownValue(manifest, "proofBackend") ?? BSC_EVM_GROTH16_BACKEND,
    proofFamily: ownValue(manifest, "proofFamily") ?? SCCP_PROOF_FAMILY_STARK_FRI,
    publicInputCount: 9,
    publicSignalNames: [...BSC_GROTH16_PUBLIC_SIGNAL_NAMES],
    verifierKeyHash: commonBody.verifierKeyHash,
    artifacts,
    transcriptValidation: {
      trustedSetup: {
        path: repoRelativePath(trustedSetupTranscript.path),
        sha256: artifacts.trustedSetupTranscript.sha256,
        blockers: trustedSetupTranscriptBlockers,
      },
      reproducibleBuild: {
        path: repoRelativePath(reproducibleBuildTranscript.path),
        sha256: artifacts.reproducibleBuildTranscript.sha256,
        blockers: reproducibleBuildTranscriptBlockers,
      },
    },
    roles: {
      semanticSccpCircuit: attestationRequestRole({
        signerRole: "semantic-sccp-circuit-reviewer",
        body: {
          schema: BSC_GROTH16_SEMANTIC_ATTESTATION_SCHEMA,
          ...commonBody,
          fullSccpMessageSemantics: true,
          sourceFinalitySemantics: true,
          destinationBindingSemantics: true,
          publicSignalDerivationSemantics: true,
          negativeCaseCoverage: true,
        },
      }),
      circuitSecurity: attestationRequestRole({
        signerRole: "circuit-security-auditor",
        body: {
          schema: BSC_GROTH16_CIRCUIT_SECURITY_ATTESTATION_SCHEMA,
          ...commonBody,
          auditResult: "pass",
          approved: true,
          criticalFindings: 0,
          highFindings: 0,
          unresolvedFindings: 0,
        },
      }),
      trustedSetup: attestationRequestRole({
        signerRole: "trusted-setup-ceremony-attester",
        body: setupBody,
        blockers: trustedSetupTranscriptBlockers,
      }),
      reproducibleBuild: attestationRequestRole({
        signerRole: "independent-reproducible-build-attester",
        body: reproducibleBody,
        blockers: reproducibleBuildTranscriptBlockers,
      }),
    },
    signingInstructions: {
      signatureSchema: BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA,
      algorithm: "ed25519",
      payloadEncoding: "canonical JSON of role.body with signature/signatures omitted",
      signedPayloadSha256:
        "sha256(canonical-json(role.body)) must equal role.signedPayloadSha256",
      finalAttestationShape:
        "copy role.body and add a signature object matching role.signatureTemplate",
      mustNotSignWhenReadyForSignatureIsFalse: true,
    },
  };
  const outPath =
    optionalPath(options, "out") ??
    join(dirname(manifestPath), `${profile.key}-bsc-groth16-attestation-request.json`);
  await writePublicJson(outPath, packageBody);
  return {
    ok: true,
    out: outPath,
    manifest: manifestPath,
    manifestSha256: packageBody.manifest.sha256,
    readyForSignature: Object.fromEntries(
      Object.entries(packageBody.roles).map(([key, role]) => [
        key,
        role.readyForSignature,
      ]),
    ),
    signedPayloadSha256: Object.fromEntries(
      Object.entries(packageBody.roles).map(([key, role]) => [
        key,
        role.signedPayloadSha256,
      ]),
    ),
  };
}

function requestManifestBlock(request) {
  const manifest = ownValue(request, "manifest");
  if (!isRecord(manifest)) {
    throw new Error("attestation request package manifest block is required.");
  }
  const manifestPath = trim(ownValue(manifest, "path"));
  if (!manifestPath) {
    throw new Error("attestation request package manifest.path is required.");
  }
  return manifest;
}

function requireRequestValue(request, key, expected, label) {
  const value = ownValue(request, key);
  if (value !== expected) {
    throw new Error(`attestation request package ${label} must be ${expected}.`);
  }
}

function requireRequestHash(request, key, expected, label) {
  const value = normalizeManifestHash(
    ownValue(request, key),
    `attestation request package ${label}`,
  );
  if (value !== expected) {
    throw new Error(`attestation request package ${label} must be ${expected}.`);
  }
}

function requestPackageArtifact(request, key, label = key) {
  return materialManifestArtifact(request, key, `request package ${label}`);
}

function requestRolePayloadHash(role, label) {
  if (!isRecord(role)) {
    throw new Error(`attestation request package ${label} role is required.`);
  }
  if (ownValue(role, "readyForSignature") !== true) {
    const blockers = ownValue(role, "blockers");
    const blockerText = Array.isArray(blockers)
      ? blockers.map((blocker) => String(blocker)).filter(Boolean).join("; ")
      : "";
    throw new Error(
      `attestation request package ${label} role is not ready for signature${
        blockerText ? `: ${blockerText}` : ""
      }.`,
    );
  }
  const blockers = ownValue(role, "blockers");
  if (!Array.isArray(blockers)) {
    throw new Error(`attestation request package ${label} blockers must be an array.`);
  }
  if (blockers.length > 0) {
    throw new Error(
      `attestation request package ${label} role must not carry blockers when ready.`,
    );
  }
  const body = ownValue(role, "body");
  if (!isRecord(body)) {
    throw new Error(`attestation request package ${label} body is required.`);
  }
  const signedPayloadSha256 = sha256Hex(attestationSignaturePayload(body));
  const declaredPayloadSha256 = normalizeManifestHash(
    ownValue(role, "signedPayloadSha256") ??
      ownValue(role, "signed_payload_sha256"),
    `attestation request package ${label} signedPayloadSha256`,
  );
  if (declaredPayloadSha256 !== signedPayloadSha256) {
    throw new Error(
      `attestation request package ${label} signedPayloadSha256 must match role body.`,
    );
  }
  const signatureTemplate = ownValue(role, "signatureTemplate");
  if (!isRecord(signatureTemplate)) {
    throw new Error(
      `attestation request package ${label} signatureTemplate is required.`,
    );
  }
  if (
    trim(ownValue(signatureTemplate, "schema")) !==
    BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA
  ) {
    throw new Error(
      `attestation request package ${label} signatureTemplate schema must be ${BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA}.`,
    );
  }
  if (trim(ownValue(signatureTemplate, "algorithm")) !== "ed25519") {
    throw new Error(
      `attestation request package ${label} signatureTemplate algorithm must be ed25519.`,
    );
  }
  const templatePayloadSha256 = normalizeManifestHash(
    ownValue(signatureTemplate, "signedPayloadSha256") ??
      ownValue(signatureTemplate, "signed_payload_sha256"),
    `attestation request package ${label} signatureTemplate signedPayloadSha256`,
  );
  if (templatePayloadSha256 !== signedPayloadSha256) {
    throw new Error(
      `attestation request package ${label} signatureTemplate signedPayloadSha256 must match role body.`,
    );
  }
  return { body, signedPayloadSha256 };
}

function validateAttestationRequestSigningInstructions(request) {
  const signingInstructions = ownValue(request, "signingInstructions");
  if (!isRecord(signingInstructions)) {
    throw new Error("attestation request package signingInstructions is required.");
  }
  if (
    trim(ownValue(signingInstructions, "signatureSchema")) !==
    BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA
  ) {
    throw new Error(
      `attestation request package signingInstructions.signatureSchema must be ${BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA}.`,
    );
  }
  if (trim(ownValue(signingInstructions, "algorithm")) !== "ed25519") {
    throw new Error(
      "attestation request package signingInstructions.algorithm must be ed25519.",
    );
  }
  if (ownValue(signingInstructions, "mustNotSignWhenReadyForSignatureIsFalse") !== true) {
    throw new Error(
      "attestation request package signingInstructions.mustNotSignWhenReadyForSignatureIsFalse must be true.",
    );
  }
}

function validateAttestationRequestCommonFields({
  request,
  manifest,
  manifestSha256,
  profile,
}) {
  if (!isRecord(request)) {
    throw new Error("attestation request package must be a JSON object.");
  }
  requireRequestValue(
    request,
    "schema",
    BSC_GROTH16_ATTESTATION_REQUEST_PACKAGE_SCHEMA,
    "schema",
  );
  requireRequestValue(request, "routeId", ROUTE_ID, "routeId");
  requireRequestValue(request, "assetKey", ASSET_KEY, "assetKey");
  requireRequestValue(request, "bscNetwork", profile.key, "bscNetwork");
  requireRequestValue(request, "chain", profile.chain, "chain");
  requireRequestValue(request, "chainIdHex", profile.chainIdHex, "chainIdHex");
  requireRequestHash(request, "networkIdHex", profile.networkIdHex, "networkIdHex");
  requireRequestValue(
    request,
    "circuitProfile",
    BSC_FULL_SCCP_CIRCUIT_PROFILE,
    "circuitProfile",
  );
  requireRequestValue(request, "publicInputCount", 9, "publicInputCount");
  const publicSignalNames = ownValue(request, "publicSignalNames");
  if (
    !Array.isArray(publicSignalNames) ||
    JSON.stringify(publicSignalNames) !==
      JSON.stringify(BSC_GROTH16_PUBLIC_SIGNAL_NAMES)
  ) {
    throw new Error(
      "attestation request package publicSignalNames must match BSC Groth16 public signals.",
    );
  }
  const manifestBlock = requestManifestBlock(request);
  const requestManifestSha256 = normalizeManifestHash(
    ownValue(manifestBlock, "sha256"),
    "attestation request package manifest.sha256",
  );
  if (requestManifestSha256 !== manifestSha256) {
    throw new Error(
      "attestation request package manifest.sha256 must match referenced material manifest.",
    );
  }
  if (
    ownValue(manifestBlock, "productionReady") !==
    (ownValue(manifest, "productionReady") === true)
  ) {
    throw new Error(
      "attestation request package manifest.productionReady must match referenced material manifest.",
    );
  }
  const requestBlockers = ownValue(manifestBlock, "productionBlockers");
  const manifestBlockers = Array.isArray(ownValue(manifest, "productionBlockers"))
    ? ownValue(manifest, "productionBlockers").map((blocker) => String(blocker))
    : [];
  if (
    !Array.isArray(requestBlockers) ||
    JSON.stringify(requestBlockers.map((blocker) => String(blocker))) !==
      JSON.stringify(manifestBlockers)
  ) {
    throw new Error(
      "attestation request package manifest.productionBlockers must match referenced material manifest.",
    );
  }
  validateMaterialManifestForAttestationRequest(manifest, profile);

  const artifacts = {};
  for (const [key, label] of [
    ["circuitSource", "circuitSource"],
    ["r1cs", "r1cs"],
    ["powersOfTau", "powersOfTau"],
    ["provingKey", "provingKey"],
    ["snarkjsVerificationKey", "snarkjsVerificationKey"],
    ["bscVerifierKey", "bscVerifierKey"],
    ["trustedSetupTranscript", "trustedSetupTranscript"],
    ["reproducibleBuildTranscript", "reproducibleBuildTranscript"],
  ]) {
    const manifestArtifact = materialManifestArtifact(manifest, key, label);
    const requestArtifact = requestPackageArtifact(request, key, label);
    if (
      requestArtifact.path !== manifestArtifact.path ||
      requestArtifact.sha256 !== manifestArtifact.sha256
    ) {
      throw new Error(
        `attestation request package ${label} artifact must match referenced material manifest.`,
      );
    }
    artifacts[key] = manifestArtifact;
  }
  validateAttestationRequestSigningInstructions(request);
  return { artifacts };
}

function requestRolePayloadForSpec(request, spec) {
  const roles = ownValue(request, "roles");
  if (!isRecord(roles)) {
    throw new Error("attestation request package roles block is required.");
  }
  const role = ownValue(roles, spec.key);
  const { body, signedPayloadSha256 } = requestRolePayloadHash(role, spec.label);
  if (trim(ownValue(role, "attestationSchema")) !== spec.expectedSchema) {
    throw new Error(
      `attestation request package ${spec.label} attestationSchema must be ${spec.expectedSchema}.`,
    );
  }
  if (trim(ownValue(body, "schema")) !== spec.expectedSchema) {
    throw new Error(
      `attestation request package ${spec.label} body schema must be ${spec.expectedSchema}.`,
    );
  }
  const unknownFields = attestationBodyUnknownFieldBlockers(
    body,
    spec.expectedSchema,
    `attestation request package ${spec.label} body`,
  );
  if (unknownFields.length > 0) {
    throw new Error(unknownFields.join("; "));
  }
  return { body, signedPayloadSha256 };
}

function attestationRequestRoleStatus(request, spec) {
  const status = {
    role: spec.key,
    label: spec.label,
    signerRole: null,
    attestationSchema: null,
    bodySchema: null,
    readyForSignature: false,
    signable: false,
    declaredBlockers: [],
    blockers: [],
    signedPayloadSha256: null,
    declaredSignedPayloadSha256: null,
  };
  const roles = ownValue(request, "roles");
  if (!isRecord(roles)) {
    status.blockers.push("attestation request package roles block is required");
    return { status, payload: null };
  }
  const role = ownValue(roles, spec.key);
  if (!isRecord(role)) {
    status.blockers.push(`attestation request package ${spec.label} role is required`);
    return { status, payload: null };
  }
  status.signerRole = trim(ownValue(role, "signerRole")) || null;
  status.attestationSchema = trim(ownValue(role, "attestationSchema")) || null;
  status.readyForSignature = ownValue(role, "readyForSignature") === true;
  const declaredBlockers = ownValue(role, "blockers");
  if (!Array.isArray(declaredBlockers)) {
    status.blockers.push(
      `attestation request package ${spec.label} blockers must be an array`,
    );
  } else {
    status.declaredBlockers = declaredBlockers
      .map((blocker) => String(blocker))
      .filter(Boolean);
  }
  if (!status.readyForSignature) {
    status.blockers.push(
      `attestation request package ${spec.label} role is not ready for signature${
        status.declaredBlockers.length > 0
          ? `: ${status.declaredBlockers.join("; ")}`
          : ""
      }`,
    );
  }
  if (status.readyForSignature && status.declaredBlockers.length > 0) {
    status.blockers.push(
      `attestation request package ${spec.label} role must not carry blockers when ready`,
    );
  }
  if (status.attestationSchema !== spec.expectedSchema) {
    status.blockers.push(
      `attestation request package ${spec.label} attestationSchema must be ${spec.expectedSchema}`,
    );
  }
  const body = ownValue(role, "body");
  if (!isRecord(body)) {
    status.blockers.push(`attestation request package ${spec.label} body is required`);
    return { status, payload: null };
  }
  status.bodySchema = trim(ownValue(body, "schema")) || null;
  if (status.bodySchema !== spec.expectedSchema) {
    status.blockers.push(
      `attestation request package ${spec.label} body schema must be ${spec.expectedSchema}`,
    );
  }
  status.blockers.push(
    ...attestationBodyUnknownFieldBlockers(
      body,
      spec.expectedSchema,
      `attestation request package ${spec.label} body`,
    ),
  );
  status.signedPayloadSha256 = sha256Hex(attestationSignaturePayload(body));
  try {
    status.declaredSignedPayloadSha256 = normalizeManifestHash(
      ownValue(role, "signedPayloadSha256") ??
        ownValue(role, "signed_payload_sha256"),
      `attestation request package ${spec.label} signedPayloadSha256`,
    );
    if (status.declaredSignedPayloadSha256 !== status.signedPayloadSha256) {
      status.blockers.push(
        `attestation request package ${spec.label} signedPayloadSha256 must match role body`,
      );
    }
  } catch (error) {
    status.blockers.push(error instanceof Error ? error.message : String(error));
  }
  const signatureTemplate = ownValue(role, "signatureTemplate");
  if (!isRecord(signatureTemplate)) {
    status.blockers.push(
      `attestation request package ${spec.label} signatureTemplate is required`,
    );
  } else {
    if (
      trim(ownValue(signatureTemplate, "schema")) !==
      BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA
    ) {
      status.blockers.push(
        `attestation request package ${spec.label} signatureTemplate schema must be ${BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA}`,
      );
    }
    if (trim(ownValue(signatureTemplate, "algorithm")) !== "ed25519") {
      status.blockers.push(
        `attestation request package ${spec.label} signatureTemplate algorithm must be ed25519`,
      );
    }
    try {
      const templatePayloadSha256 = normalizeManifestHash(
        ownValue(signatureTemplate, "signedPayloadSha256") ??
          ownValue(signatureTemplate, "signed_payload_sha256"),
        `attestation request package ${spec.label} signatureTemplate signedPayloadSha256`,
      );
      if (templatePayloadSha256 !== status.signedPayloadSha256) {
        status.blockers.push(
          `attestation request package ${spec.label} signatureTemplate signedPayloadSha256 must match role body`,
        );
      }
    } catch (error) {
      status.blockers.push(error instanceof Error ? error.message : String(error));
    }
  }
  status.signable =
    status.readyForSignature &&
    status.declaredBlockers.length === 0 &&
    status.blockers.length === 0;
  return {
    status,
    payload: status.signable
      ? { body, signedPayloadSha256: status.signedPayloadSha256 }
      : null,
  };
}

function validateAttestationRequestPackageForFinalize({
  request,
  manifest,
  manifestSha256,
  profile,
}) {
  const { artifacts } = validateAttestationRequestCommonFields({
    request,
    manifest,
    manifestSha256,
    profile,
  });

  const rolePayloads = {};
  for (const spec of BSC_GROTH16_ATTESTATION_ROLE_SPECS) {
    const { body, signedPayloadSha256 } = requestRolePayloadForSpec(request, spec);
    rolePayloads[spec.key] = { body, signedPayloadSha256 };
  }
  return { artifacts, rolePayloads };
}

async function readEd25519PrivateKey(pathName) {
  const privateKeyPath = await assertReadableRegularFile(
    pathName,
    "Ed25519 attestation private key",
  );
  let privateKey;
  try {
    privateKey = createPrivateKey(await readFile(privateKeyPath));
  } catch (error) {
    throw new Error(
      `Ed25519 attestation private key could not be parsed: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  }
  if (privateKey.asymmetricKeyType !== "ed25519") {
    throw new Error(
      `Ed25519 attestation private key must be ed25519, got ${privateKey.asymmetricKeyType ?? "unknown"}.`,
    );
  }
  const publicKey = createPublicKey(privateKey);
  const publicKeyPem = publicKey.export({ format: "pem", type: "spki" });
  const signerFingerprint = sha256Hex(
    publicKey.export({ format: "der", type: "spki" }),
  );
  return { privateKey, privateKeyPath, publicKeyPem, signerFingerprint };
}

function signedAttestationRecord({ body, privateKey, publicKeyPem, signerFingerprint }) {
  const payload = attestationSignaturePayload(body);
  return {
    ...body,
    signature: {
      schema: BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA,
      algorithm: "ed25519",
      signerFingerprint,
      publicKeyPem,
      signedPayloadSha256: sha256Hex(payload),
      signature: signDetachedPayload(null, payload, privateKey).toString("base64"),
    },
  };
}

export async function signBscGroth16AttestationRole(options = {}) {
  const requestPath = await assertReadableRegularFile(
    requiredOption(
      options,
      ["request", "attestation-request", "request-package"],
      "attestation signing",
    ),
    "BSC Groth16 attestation request package",
  );
  const request = await readJson(
    requestPath,
    "BSC Groth16 attestation request package",
  );
  const secretReason = unsafeSecretReason(
    request,
    "BSC Groth16 attestation request package",
  );
  if (secretReason) {
    throw new Error(secretReason);
  }
  const profile = normalizeBscNetworkProfile(
    ownValue(options, "bsc-network") ??
      ownValue(options, "network") ??
      ownValue(request, "bscNetwork"),
  );
  const manifestBlock = requestManifestBlock(request);
  const manifestPath = await resolveManifestArtifactPath(
    requestPath,
    { path: trim(ownValue(manifestBlock, "path")) },
    "BSC Groth16 material manifest",
  );
  const manifestSha256 = await fileSha256(manifestPath);
  const manifest = await readJson(manifestPath, "BSC Groth16 material manifest");
  const manifestSecretReason = unsafeSecretReason(
    manifest,
    "BSC Groth16 material manifest",
  );
  if (manifestSecretReason) {
    throw new Error(manifestSecretReason);
  }
  validateAttestationRequestCommonFields({
    request,
    manifest,
    manifestSha256,
    profile,
  });
  const roleKey = normalizeAttestationRole(
    requiredOption(options, ["role", "attestation-role"], "attestation signing"),
  );
  const spec = attestationRoleSpec(roleKey);
  const rolePayload = requestRolePayloadForSpec(request, spec);
  const {
    privateKey,
    privateKeyPath,
    publicKeyPem,
    signerFingerprint,
  } = await readEd25519PrivateKey(
    requiredOption(
      options,
      ["private-key-pem", "private-key-file", "ed25519-private-key"],
      "attestation signing",
    ),
  );
  const explicitFingerprint = ownValue(options, "signer-fingerprint");
  if (explicitFingerprint !== undefined && trim(explicitFingerprint) !== "") {
    const normalized = normalizeSignerFingerprint(
      explicitFingerprint,
      "attestation signing signer fingerprint",
    );
    if (normalized !== signerFingerprint) {
      throw new Error(
        "attestation signing signer fingerprint must match the supplied Ed25519 private key.",
      );
    }
  }
  const record = signedAttestationRecord({
    body: rolePayload.body,
    privateKey,
    publicKeyPem,
    signerFingerprint,
  });
  const outPath =
    optionalPath(options, "out") ??
    join(dirname(requestPath), `${profile.key}-bsc-groth16-${roleKey}-attestation.json`);
  if (resolve(outPath) === privateKeyPath) {
    throw new Error("attestation signing output must not overwrite the private key file.");
  }
  await writePublicJson(outPath, record);
  return {
    ok: true,
    role: roleKey,
    signerFingerprint,
    out: outPath,
    request: requestPath,
    requestSha256: await fileSha256(requestPath),
    requestManifest: manifestPath,
    requestManifestSha256: manifestSha256,
    signedPayloadSha256: rolePayload.signedPayloadSha256,
    attestationSha256: await fileSha256(outPath),
  };
}

function signedAttestationMatchesRequestBlockers(entry, rolePayload, label) {
  if (!entry) {
    return [`${label} attestation file is required`];
  }
  if (entry.readError) {
    return [`${label} attestation is not valid JSON: ${entry.readError}`];
  }
  if (!isRecord(entry.record)) {
    return [`${label} attestation must be a JSON object`];
  }
  const actualPayloadSha256 = sha256Hex(attestationSignaturePayload(entry.record));
  return actualPayloadSha256 === rolePayload.signedPayloadSha256
    ? []
    : [
        `${label} signed attestation body must match attestation request package signedPayloadSha256`,
      ];
}

async function transcriptValidationStatus({ manifestPath, artifacts, manifest }) {
  const selfChecks = ownValue(manifest, "selfChecks");
  const snarkjsSelfCheck = isRecord(selfChecks)
    ? ownValue(selfChecks, "snarkjs")
    : null;
  const buildTranscriptStatus = async (key, label, validator) => {
    const artifact = artifacts[key];
    const status = {
      path: artifact?.path ?? null,
      sha256: artifact?.sha256 ?? null,
      resolvedPath: null,
      blockers: [],
    };
    if (!artifact) {
      status.blockers.push(`${label} artifact is required`);
      return status;
    }
    try {
      const resolved = await resolveManifestArtifactFile(
        manifestPath,
        artifact,
        label,
      );
      status.resolvedPath = repoRelativePath(resolved);
      status.blockers.push(...(await validator(resolved)));
    } catch (error) {
      status.blockers.push(error instanceof Error ? error.message : String(error));
    }
    return status;
  };
  return {
    trustedSetup: await buildTranscriptStatus(
      "trustedSetupTranscript",
      "trusted setup transcript",
      validateTrustedSetupTranscript,
    ),
    reproducibleBuild: await buildTranscriptStatus(
      "reproducibleBuildTranscript",
      "reproducible build transcript",
      (pathName) => validateReproducibleBuildTranscript(pathName, snarkjsSelfCheck),
    ),
  };
}

function flattenedTranscriptBlockers(transcriptValidation) {
  return Object.entries(transcriptValidation).flatMap(([key, value]) =>
    (Array.isArray(value?.blockers) ? value.blockers : []).map(
      (blocker) => `${key}: ${blocker}`,
    ),
  );
}

function attestationStatusNextActions({
  requestProblems,
  roleStatuses,
  missingSignedRoles,
  signedRoleProblems,
  materialValidationBlockers,
  transcriptBlockers,
  trustedSignerFingerprints,
  readyToFinalize,
}) {
  if (readyToFinalize) {
    return [
      "Run finalize-attestations with the same request, signed role files, and trusted signer fingerprints.",
    ];
  }
  const actions = [];
  if (requestProblems.length > 0) {
    actions.push("Regenerate the attestation-request package from the current material manifest.");
  }
  if (
    Object.values(roleStatuses).some(
      (status) => !status.readyForSignature || status.declaredBlockers.length > 0,
    ) ||
    transcriptBlockers.length > 0
  ) {
    actions.push(
      "Replace local-only setup/rebuild evidence or rerun attestation-request after publishing production transcript evidence.",
    );
  }
  if (missingSignedRoles.length > 0) {
    const missingReadyRoles = missingSignedRoles.filter(
      (role) => roleStatuses[role]?.signable,
    );
    const missingBlockedRoles = missingSignedRoles.filter(
      (role) => !roleStatuses[role]?.signable,
    );
    if (missingReadyRoles.length > 0) {
      actions.push(
        `Sign the missing ready role payloads: ${missingReadyRoles.join(", ")}.`,
      );
    }
    if (missingBlockedRoles.length > 0) {
      actions.push(
        `Do not sign blocked role payloads until their blockers are resolved: ${missingBlockedRoles.join(", ")}.`,
      );
    }
  }
  if (trustedSignerFingerprints.length === 0) {
    actions.push("Pass --trusted-attestation-signer with every allowed Ed25519 signer fingerprint.");
  }
  if (signedRoleProblems.length > 0 || materialValidationBlockers.length > 0) {
    actions.push(
      "Discard forged or stale signed role files and re-sign the exact request role bodies with role-separated trusted signers.",
    );
  }
  return [...new Set(actions)];
}

export async function auditBscGroth16AttestationStatus(options = {}) {
  const requestPath = await assertReadableRegularFile(
    requiredOption(
      options,
      ["request", "attestation-request", "request-package"],
      "attestation status",
    ),
    "BSC Groth16 attestation request package",
  );
  const request = await readJson(
    requestPath,
    "BSC Groth16 attestation request package",
  );
  const secretReason = unsafeSecretReason(
    request,
    "BSC Groth16 attestation request package",
  );
  if (secretReason) {
    throw new Error(secretReason);
  }
  const profile = normalizeBscNetworkProfile(
    ownValue(options, "bsc-network") ??
      ownValue(options, "network") ??
      ownValue(request, "bscNetwork"),
  );
  const manifestBlock = requestManifestBlock(request);
  const manifestPath = await resolveManifestArtifactPath(
    requestPath,
    { path: trim(ownValue(manifestBlock, "path")) },
    "BSC Groth16 material manifest",
  );
  const manifestSha256 = await fileSha256(manifestPath);
  const manifest = await readJson(manifestPath, "BSC Groth16 material manifest");
  const manifestSecretReason = unsafeSecretReason(
    manifest,
    "BSC Groth16 material manifest",
  );
  if (manifestSecretReason) {
    throw new Error(manifestSecretReason);
  }
  const requestProblems = [];
  let common = null;
  try {
    common = validateAttestationRequestCommonFields({
      request,
      manifest,
      manifestSha256,
      profile,
    });
  } catch (error) {
    requestProblems.push(error instanceof Error ? error.message : String(error));
  }
  const roleStatuses = {};
  const rolePayloads = {};
  for (const spec of BSC_GROTH16_ATTESTATION_ROLE_SPECS) {
    const { status, payload } = attestationRequestRoleStatus(request, spec);
    roleStatuses[spec.key] = status;
    if (payload) {
      rolePayloads[spec.key] = payload;
    }
  }
  const attestations = await buildAttestationReferences(options);
  const trustedSignerFingerprints = parseTrustedSignerFingerprints(options);
  const missingSignedRoles = BSC_GROTH16_ATTESTATION_ROLE_SPECS
    .filter((spec) => !attestations[spec.key])
    .map((spec) => spec.key);
  const signedRoleProblems = [];
  for (const spec of BSC_GROTH16_ATTESTATION_ROLE_SPECS) {
    const entry = attestations[spec.key];
    if (!entry) {
      continue;
    }
    const rolePayload = rolePayloads[spec.key];
    if (!rolePayload) {
      signedRoleProblems.push(
        `${spec.label} signed attestation was supplied, but the request role is not ready for signature`,
      );
      continue;
    }
    signedRoleProblems.push(
      ...signedAttestationMatchesRequestBlockers(entry, rolePayload, spec.label),
    );
  }
  const manifestProductionBlockers = Array.isArray(
    ownValue(manifest, "productionBlockers"),
  )
    ? ownValue(manifest, "productionBlockers").map((blocker) => String(blocker))
    : [];
  const transcriptValidation = common
    ? await transcriptValidationStatus({
        manifestPath,
        artifacts: common.artifacts,
        manifest,
      })
    : {
        trustedSetup: { path: null, sha256: null, resolvedPath: null, blockers: [] },
        reproducibleBuild: { path: null, sha256: null, resolvedPath: null, blockers: [] },
      };
  const transcriptBlockers = flattenedTranscriptBlockers(transcriptValidation);
  const materialValidationBlockers = common
    ? validateAttestationsForMaterial(
        attestations,
        {
          profile,
          circuitProfile: BSC_FULL_SCCP_CIRCUIT_PROFILE,
          publicSignalNames: [...BSC_GROTH16_PUBLIC_SIGNAL_NAMES],
          verifierKeyHash: normalizeManifestHash(
            ownValue(manifest, "verifierKeyHash"),
            "material manifest verifierKeyHash",
          ),
          artifacts: common.artifacts,
          selfChecks: ownValue(manifest, "selfChecks"),
        },
        trustedSignerFingerprints,
      )
    : [];
  const roleProblems = Object.values(roleStatuses).flatMap((status) =>
    status.blockers.map((blocker) => `${status.role}: ${blocker}`),
  );
  const requestReadyForSignature = Object.fromEntries(
    Object.entries(roleStatuses).map(([key, value]) => [key, value.signable]),
  );
  const allRolesSignable = Object.values(roleStatuses).every(
    (status) => status.signable,
  );
  const problems = [
    ...requestProblems,
    ...roleProblems,
    ...transcriptBlockers,
    ...signedRoleProblems,
    ...materialValidationBlockers,
    ...missingSignedRoles.map((role) => `${role} signed attestation file is missing`),
  ];
  const readyToFinalize =
    requestProblems.length === 0 &&
    allRolesSignable &&
    transcriptBlockers.length === 0 &&
    missingSignedRoles.length === 0 &&
    signedRoleProblems.length === 0 &&
    materialValidationBlockers.length === 0;
  return {
    ok: true,
    readyToFinalize,
    requestValid: requestProblems.length === 0,
    request: requestPath,
    requestSha256: await fileSha256(requestPath),
    requestManifest: manifestPath,
    requestManifestSha256: manifestSha256,
    bscNetwork: profile.key,
    manifest: {
      path: repoRelativePath(manifestPath),
      productionReady: ownValue(manifest, "productionReady") === true,
      productionBlockers: manifestProductionBlockers,
      verifierKeyHash: trim(ownValue(manifest, "verifierKeyHash")),
    },
    trustedSignerFingerprints,
    requestReadyForSignature,
    roles: roleStatuses,
    transcriptValidation,
    signedRoles: publicAttestationReferences(attestations),
    missingSignedRoles,
    signedRoleProblems,
    materialValidationBlockers,
    problems,
    problemCount: problems.length,
    nextActions: attestationStatusNextActions({
      requestProblems,
      roleStatuses,
      missingSignedRoles,
      signedRoleProblems,
      materialValidationBlockers,
      transcriptBlockers,
      trustedSignerFingerprints,
      readyToFinalize,
    }),
  };
}

export async function finalizeBscGroth16Attestations(options = {}) {
  const requestPath = await assertReadableRegularFile(
    requiredOption(
      options,
      ["request", "attestation-request", "request-package"],
      "attestation finalization",
    ),
    "BSC Groth16 attestation request package",
  );
  const request = await readJson(
    requestPath,
    "BSC Groth16 attestation request package",
  );
  const secretReason = unsafeSecretReason(
    request,
    "BSC Groth16 attestation request package",
  );
  if (secretReason) {
    throw new Error(secretReason);
  }
  const profile = normalizeBscNetworkProfile(
    ownValue(options, "bsc-network") ??
      ownValue(options, "network") ??
      ownValue(request, "bscNetwork"),
  );
  const manifestBlock = requestManifestBlock(request);
  const manifestPath = await resolveManifestArtifactPath(
    requestPath,
    { path: trim(ownValue(manifestBlock, "path")) },
    "BSC Groth16 material manifest",
  );
  const manifestSha256 = await fileSha256(manifestPath);
  const manifest = await readJson(manifestPath, "BSC Groth16 material manifest");
  const manifestSecretReason = unsafeSecretReason(
    manifest,
    "BSC Groth16 material manifest",
  );
  if (manifestSecretReason) {
    throw new Error(manifestSecretReason);
  }
  const { artifacts, rolePayloads } = validateAttestationRequestPackageForFinalize({
    request,
    manifest,
    manifestSha256,
    profile,
  });
  const resolvedArtifacts = {
    r1cs: await resolveManifestArtifactFile(manifestPath, artifacts.r1cs, "R1CS"),
    powersOfTau: await resolveManifestArtifactFile(
      manifestPath,
      artifacts.powersOfTau,
      "Powers of Tau",
    ),
    provingKey: await resolveManifestArtifactFile(
      manifestPath,
      artifacts.provingKey,
      "proving key",
    ),
    snarkjsVerificationKey: await resolveManifestArtifactFile(
      manifestPath,
      artifacts.snarkjsVerificationKey,
      "SnarkJS verification key",
    ),
    circuitSource: await resolveManifestArtifactFile(
      manifestPath,
      artifacts.circuitSource,
      "circuit source",
    ),
    trustedSetupTranscript: await resolveManifestArtifactFile(
      manifestPath,
      artifacts.trustedSetupTranscript,
      "trusted setup transcript",
    ),
    reproducibleBuildTranscript: await resolveManifestArtifactFile(
      manifestPath,
      artifacts.reproducibleBuildTranscript,
      "reproducible build transcript",
    ),
  };
  const attestations = await buildAttestationReferences(options);
  const matchBlockers = [];
  for (const spec of BSC_GROTH16_ATTESTATION_ROLE_SPECS) {
    matchBlockers.push(
      ...signedAttestationMatchesRequestBlockers(
        attestations[spec.key],
        rolePayloads[spec.key],
        spec.label,
      ),
    );
  }
  if (matchBlockers.length > 0) {
    throw new Error(
      `signed attestations do not match request package: ${matchBlockers.join("; ")}`,
    );
  }
  const result = await materializeBscGroth16Material({
    ...options,
    "bsc-network": profile.key,
    r1cs: resolvedArtifacts.r1cs,
    ptau: resolvedArtifacts.powersOfTau,
    zkey: resolvedArtifacts.provingKey,
    "snarkjs-verifier-key": resolvedArtifacts.snarkjsVerificationKey,
    "circuit-source": resolvedArtifacts.circuitSource,
    "trusted-setup-transcript": resolvedArtifacts.trustedSetupTranscript,
    "reproducible-build-transcript": resolvedArtifacts.reproducibleBuildTranscript,
    "out-dir": resolve(ownValue(options, "out-dir") ?? dirname(requestPath)),
  });
  if (result.productionReady !== true) {
    const blockers = Array.isArray(result.productionBlockers)
      ? result.productionBlockers.map((blocker) => String(blocker)).filter(Boolean)
      : [];
    throw new Error(
      "attestation finalization did not produce productionReady material" +
        (blockers.length > 0 ? `: ${blockers.join("; ")}` : "."),
    );
  }
  return {
    ...result,
    request: requestPath,
    requestSha256: await fileSha256(requestPath),
    requestManifest: manifestPath,
    requestManifestSha256: manifestSha256,
    requestSignedPayloadSha256: Object.fromEntries(
      Object.entries(rolePayloads).map(([key, value]) => [
        key,
        value.signedPayloadSha256,
      ]),
    ),
  };
}

export async function preflightBscGroth16Material(options = {}) {
  const profile = normalizeBscNetworkProfile(
    ownValue(options, "bsc-network") ?? ownValue(options, "network") ?? "testnet",
  );
  const outDir = resolve(ownValue(options, "out-dir") ?? defaultMaterialOut(profile));
  const circuitProfile = trim(
    ownValue(options, "circuit-profile") ?? BSC_FULL_SCCP_CIRCUIT_PROFILE,
  );
  if (
    circuitProfile !== BSC_SIGNAL_BINDING_CIRCUIT_PROFILE &&
    circuitProfile !== BSC_FULL_SCCP_CIRCUIT_PROFILE
  ) {
    throw new Error(
      `--circuit-profile must be ${BSC_SIGNAL_BINDING_CIRCUIT_PROFILE} or ${BSC_FULL_SCCP_CIRCUIT_PROFILE}.`,
    );
  }
  const circomBin = commandValue(options, "circom-bin", "circom2");
  const snarkjsBin = commandValue(options, "snarkjs-bin", "snarkjs");
  const toolchainRoot = groth16ToolchainRoot(options);
  const displayCircomBin = displayCommandValue(circomBin);
  const displaySnarkjsBin = displayCommandValue(snarkjsBin);
  const paths = bscGroth16ArtifactPaths({ outDir, profile, circuitProfile });
  const attestationRequestPath = join(
    outDir,
    `${profile.key}-bsc-groth16-attestation-request.json`,
  );
  const toolchain = {
    circom: await commandProbe(circomBin, ["--help"]),
    snarkjs: await commandProbe(snarkjsBin, ["r1cs", "info", "--help"]),
  };
  const problems = [];
  if (!toolchain.circom.ok) {
    problems.push(`Circom compiler probe failed: ${toolchain.circom.error}`);
  }
  if (!toolchain.snarkjs.ok) {
    problems.push(`SnarkJS probe failed: ${toolchain.snarkjs.error}`);
  }
  const artifactEntries = await Promise.all(
    Object.entries(paths).map(async ([key, pathName]) => [key, await fileProbe(pathName)]),
  );
  const artifacts = Object.fromEntries(artifactEntries);
  const missing = [];
  for (const [key, artifact] of Object.entries(artifacts)) {
    if (!artifact.present) {
      missing.push(key);
      continue;
    }
    if (artifact.symbolicLink) {
      problems.push(`${key} must not be a symbolic link: ${artifact.path}`);
    }
    if (!artifact.regularFile) {
      problems.push(`${key} must be a regular file: ${artifact.path}`);
    }
    if (artifact.error) {
      problems.push(`${key} could not be inspected: ${artifact.error}`);
    }
  }
  for (const key of missing) {
    problems.push(`${key} artifact is missing: ${artifacts[key].path}`);
  }

  if (artifacts.circuitSource.present && artifacts.circuitSource.regularFile) {
    try {
      const sourceCheck =
        circuitProfile === BSC_FULL_SCCP_CIRCUIT_PROFILE
          ? await fullCircuitSourceCheck(paths.circuitSource)
          : null;
      if (sourceCheck) {
        artifacts.circuitSource.checks = sourceCheck.checks;
        problems.push(...sourceCheck.blockers);
      }
    } catch (error) {
      problems.push(
        `circuit source self-check failed: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
    }
  }

  if (artifacts.r1cs.present && artifacts.r1cs.regularFile) {
    try {
      const blockers = await snarkjsArtifactBlockers(
        paths.r1cs,
        "R1CS",
        SNARKJS_R1CS_MAGIC,
        PRODUCTION_SNARKJS_R1CS_MIN_BYTES,
      );
      problems.push(...blockers);
      try {
        artifacts.r1cs.r1csHeader = await readSnarkjsR1csHeader(paths.r1cs);
      } catch (error) {
        problems.push(
          `R1CS header self-check failed: ${
            error instanceof Error ? error.message : String(error)
          }`,
        );
      }
    } catch (error) {
      problems.push(
        `R1CS artifact self-check failed: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
    }
  }

  if (artifacts.provingKey.present && artifacts.provingKey.regularFile) {
    try {
      problems.push(
        ...(await snarkjsArtifactBlockers(
          paths.provingKey,
          "zkey",
          SNARKJS_ZKEY_MAGIC,
          PRODUCTION_SNARKJS_ZKEY_MIN_BYTES,
        )),
      );
    } catch (error) {
      problems.push(
        `zkey artifact self-check failed: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
    }
  }

  let snarkjsVerifierMaterial = null;
  if (
    artifacts.snarkjsVerificationKey.present &&
    artifacts.snarkjsVerificationKey.regularFile
  ) {
    try {
      snarkjsVerifierMaterial = snarkjsVerificationKeyToBscVerifierMaterial(
        await readJson(paths.snarkjsVerificationKey, "SnarkJS verification key"),
        { bscNetwork: profile.key },
      );
      artifacts.snarkjsVerificationKey.verifierKeyHash =
        snarkjsVerifierMaterial.verifierKeyHash;
    } catch (error) {
      problems.push(
        `SnarkJS verification key self-check failed: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
    }
  }

  let bscVerifierMaterial = null;
  if (artifacts.bscVerifierKey.present && artifacts.bscVerifierKey.regularFile) {
    try {
      bscVerifierMaterial = normalizeVerifierMaterial(
        await readJson(paths.bscVerifierKey, "BSC Groth16 verifier key"),
        profile,
      );
      artifacts.bscVerifierKey.verifierKeyHash =
        bscVerifierMaterial.expectedVerifierKeyHash;
    } catch (error) {
      problems.push(
        `BSC verifier key self-check failed: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
    }
  }
  if (
    snarkjsVerifierMaterial &&
    bscVerifierMaterial &&
    snarkjsVerifierMaterial.verifierKeyHash !==
      bscVerifierMaterial.expectedVerifierKeyHash
  ) {
    problems.push(
      "SnarkJS verification key and BSC verifier key hashes must match.",
    );
  }

  if (artifacts.manifest.present && artifacts.manifest.regularFile) {
    try {
      const manifest = await readJson(paths.manifest, "BSC Groth16 material manifest");
      artifacts.manifest.productionReady = manifest.productionReady === true;
      artifacts.manifest.verifierKeyHash = trim(ownValue(manifest, "verifierKeyHash"));
      if (
        trim(ownValue(manifest, "schema")) !==
        BSC_GROTH16_MATERIAL_MANIFEST_SCHEMA
      ) {
        problems.push(
          `material manifest schema must be ${BSC_GROTH16_MATERIAL_MANIFEST_SCHEMA}`,
        );
      }
      if (trim(ownValue(manifest, "bscNetwork")) !== profile.key) {
        problems.push(`material manifest bscNetwork must be ${profile.key}`);
      }
      if (trim(ownValue(manifest, "circuitProfile")) !== circuitProfile) {
        problems.push(`material manifest circuitProfile must be ${circuitProfile}`);
      }
      const manifestVerifierKeyHash = trim(ownValue(manifest, "verifierKeyHash"));
      if (
        bscVerifierMaterial &&
        manifestVerifierKeyHash !== bscVerifierMaterial.expectedVerifierKeyHash
      ) {
        problems.push(
          "material manifest verifierKeyHash must match BSC verifier key hash.",
        );
      }
      if (manifest.productionReady !== true) {
        problems.push("material manifest is not productionReady.");
      }
      if (
        Array.isArray(manifest.productionBlockers) &&
        manifest.productionBlockers.length > 0
      ) {
        for (const blocker of manifest.productionBlockers) {
          problems.push(`material manifest production blocker: ${blocker}`);
        }
      }
    } catch (error) {
      problems.push(
        `material manifest self-check failed: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
    }
  }

  if (
    artifacts.proofSelfTest.present &&
    artifacts.proofSelfTest.regularFile &&
    artifacts.manifest.present &&
    artifacts.manifest.regularFile
  ) {
    try {
      const proofSelfTestBlockers = await validateProofSelfTestReport({
        reportPath: paths.proofSelfTest,
        profile,
        circuitProfile,
        manifestPath: paths.manifest,
        paths,
      });
      if (proofSelfTestBlockers.length > 0) {
        for (const blocker of proofSelfTestBlockers) {
          problems.push(`proof self-test report blocker: ${blocker}`);
        }
      } else {
        artifacts.proofSelfTest.verified = true;
      }
    } catch (error) {
      problems.push(
        `proof self-test report self-check failed: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
    }
  }

  const attestationRequest = await fileProbe(attestationRequestPath);
  artifacts.attestationRequest = attestationRequest;
  if (attestationRequest.present && attestationRequest.regularFile) {
    try {
      const status = await auditBscGroth16AttestationStatus({
        ...options,
        "bsc-network": profile.key,
        request: attestationRequestPath,
      });
      artifacts.attestationRequest.status = {
        readyToFinalize: status.readyToFinalize,
        requestValid: status.requestValid,
        problemCount: status.problemCount,
        firstProblems: status.problems.slice(0, 8),
        requestReadyForSignature: status.requestReadyForSignature,
        missingSignedRoles: status.missingSignedRoles,
        nextActions: status.nextActions,
      };
      if (!status.requestValid) {
        problems.push(
          `attestation request self-check failed: ${
            status.problems[0] ?? "request package is invalid"
          }`,
        );
      }
    } catch (error) {
      problems.push(
        `attestation request self-check failed: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
    }
  }

  const artifactReady = missing.length === 0 && !problems.some((problem) => !/^Circom compiler probe failed:|^SnarkJS probe failed:/u.test(problem));
  const toolchainReady = toolchain.circom.ok && toolchain.snarkjs.ok;
  const ready = toolchainReady && artifactReady && problems.length === 0;
  return {
    ok: true,
    ready,
    toolchainReady,
    artifactReady,
    bscNetwork: profile.key,
    circuitProfile,
    outDir: repoRelativePath(outDir),
    toolchainRoot: repoRelativePath(toolchainRoot.path),
    toolchainRootExplicit: toolchainRoot.explicit,
    toolchain,
    artifacts,
    missing,
    problems,
    commands: {
      compile: `${displayCircomBin} ${repoRelativePath(paths.circuitSource)} --r1cs --wasm --sym -l <node_modules> -o ${repoRelativePath(outDir)}`,
      r1csInfo: `${displaySnarkjsBin} r1cs info ${repoRelativePath(paths.r1cs)}`,
      setup: `${displaySnarkjsBin} groth16 setup ${repoRelativePath(paths.r1cs)} <powersOfTau28_hez_final_22.ptau> ${repoRelativePath(join(outDir, `${circuitProfile}.0000.zkey`))}`,
      exportVerificationKey: `${displaySnarkjsBin} zkey export verificationkey ${repoRelativePath(paths.provingKey)} ${repoRelativePath(paths.snarkjsVerificationKey)}`,
      materialize: `node scripts/sccp_bsc_groth16_material.mjs materialize --bsc-network ${profile.key} --r1cs ${repoRelativePath(paths.r1cs)} --zkey ${repoRelativePath(paths.provingKey)} --ptau <powersOfTau28_hez_final_22.ptau> --snarkjs-verifier-key ${repoRelativePath(paths.snarkjsVerificationKey)} --circuit-source ${repoRelativePath(paths.circuitSource)} --out-dir ${repoRelativePath(outDir)} --trusted-setup-transcript <json> --reproducible-build-transcript <json>`,
      proofSelfTest: `node scripts/sccp_bsc_groth16_material.mjs proof-self-test --manifest ${repoRelativePath(paths.manifest)} --witness-wasm ${repoRelativePath(paths.witnessWasm)}${profile.key === "testnet" ? " --allow-unready-candidate true" : " --allow-unready-mainnet-candidate true"} --out ${repoRelativePath(join(outDir, `${profile.key}-bsc-groth16-proof-self-test.json`))}`,
      attestationRequest: `node scripts/sccp_bsc_groth16_material.mjs attestation-request --manifest ${repoRelativePath(paths.manifest)} --toolchain-sha256 <0x...> --out ${repoRelativePath(join(outDir, `${profile.key}-bsc-groth16-attestation-request.json`))}`,
      signAttestation: `node scripts/sccp_bsc_groth16_material.mjs sign-attestation --request ${repoRelativePath(join(outDir, `${profile.key}-bsc-groth16-attestation-request.json`))} --role semanticSccpCircuit --private-key-pem <ed25519-private-key.pem> --out <signed-role-attestation.json>`,
      attestationStatus: `node scripts/sccp_bsc_groth16_material.mjs attestation-status --request ${repoRelativePath(join(outDir, `${profile.key}-bsc-groth16-attestation-request.json`))} --semantic-attestation <semantic-sccp-circuit-attestation.json> --circuit-security-attestation <circuit-security-audit.json> --trusted-setup-attestation <trusted-setup-ceremony.json> --reproducible-build-attestation <reproducible-build-attestation.json> --trusted-attestation-signer <0x...>`,
      finalizeAttestations: `node scripts/sccp_bsc_groth16_material.mjs finalize-attestations --request ${repoRelativePath(join(outDir, `${profile.key}-bsc-groth16-attestation-request.json`))} --semantic-attestation <semantic-sccp-circuit-attestation.json> --circuit-security-attestation <circuit-security-audit.json> --trusted-setup-attestation <trusted-setup-ceremony.json> --reproducible-build-attestation <reproducible-build-attestation.json> --trusted-attestation-signer <0x...> --out-dir ${repoRelativePath(outDir)}`,
    },
  };
}

function usage() {
  return `Usage:
  node scripts/sccp_bsc_groth16_material.mjs generate --bsc-network testnet --ptau <phase2.ptau> [--circuit-profile ${BSC_SIGNAL_BINDING_CIRCUIT_PROFILE}|${BSC_FULL_SCCP_CIRCUIT_PROFILE}] [--circuit-source <full-message.circom>] [--out-dir ${DEFAULT_GENERATED_MATERIAL_OUT}/testnet] [--toolchain-root ${DEFAULT_GROTH16_TOOLCHAIN_ROOT}] [--circom-bin circom2] [--snarkjs-bin snarkjs]
  node scripts/sccp_bsc_groth16_material.mjs generate --bsc-network testnet --create-local-ptau-power 8 --allow-local-testnet-setup true [--out-dir ${DEFAULT_GENERATED_MATERIAL_OUT}/testnet] [--toolchain-root ${DEFAULT_GROTH16_TOOLCHAIN_ROOT}]
  node scripts/sccp_bsc_groth16_material.mjs materialize --bsc-network testnet|mainnet --r1cs <file.r1cs> --zkey <file.zkey> --ptau <powersOfTau28_hez_final_22.ptau> --snarkjs-verifier-key <verification_key.json> [--circuit-source <full-message.circom>] --trusted-setup-transcript <json> --reproducible-build-transcript <json> [--out-dir ${DEFAULT_NATIVE_EVM_PROVER_ARTIFACT_ROOT}/testnet]
  node scripts/sccp_bsc_groth16_material.mjs proof-self-test --manifest <testnet|mainnet-bsc-groth16-material.manifest.json> [--witness-wasm <circuit.wasm>] [--snarkjs-bin snarkjs] [--allow-unready-candidate true|--allow-unready-mainnet-candidate true] [--out <proof-self-test.json>]
  node scripts/sccp_bsc_groth16_material.mjs attestation-request --manifest <testnet-bsc-groth16-material.manifest.json> [--toolchain-sha256 <0x...>] [--out <request.json>]
  node scripts/sccp_bsc_groth16_material.mjs sign-attestation --request <attestation-request.json> --role semanticSccpCircuit|circuitSecurity|trustedSetup|reproducibleBuild --private-key-pem <ed25519-private-key.pem> [--signer-fingerprint <0x...>] [--out <signed-role-attestation.json>]
  node scripts/sccp_bsc_groth16_material.mjs attestation-status --request <attestation-request.json> --semantic-attestation <json> --circuit-security-attestation <json> --trusted-setup-attestation <json> --reproducible-build-attestation <json> --trusted-attestation-signer <0x...>
  node scripts/sccp_bsc_groth16_material.mjs finalize-attestations --request <attestation-request.json> --semantic-attestation <json> --circuit-security-attestation <json> --trusted-setup-attestation <json> --reproducible-build-attestation <json> --trusted-attestation-signer <0x...> [--out-dir <dir>]
  node scripts/sccp_bsc_groth16_material.mjs preflight --bsc-network testnet|mainnet [--circuit-profile ${BSC_FULL_SCCP_CIRCUIT_PROFILE}] [--out-dir ${DEFAULT_GENERATED_MATERIAL_OUT}/testnet] [--toolchain-root ${DEFAULT_GROTH16_TOOLCHAIN_ROOT}] [--circom-bin circom2] [--snarkjs-bin snarkjs]

The generate command creates real Circom/SnarkJS Groth16 candidate material for
the BSC verifier's 9 public signal words using circuit profile
${BSC_SIGNAL_BINDING_CIRCUIT_PROFILE} by default. Full-message generation
uses ${DEFAULT_BSC_FULL_MESSAGE_CIRCUIT_SOURCE} unless --circuit-source is
provided. Materialize records circuit-source self-checks and fails
closed unless the source constrains all 9 labeled Keccak public signals.
Materialize is an unsigned candidate-material step on the CLI. Generated local
setup material is not production-ready unless the full SCCP circuit semantics
and ceremony/build evidence are supplied through attestation-request and
finalize-attestations. Trusted setup and reproducible-build attestations must
bind to concrete transcript artifacts, and production attestations must carry
detached Ed25519 signatures from a configured trusted signer fingerprint. The
proof-self-test command runs a deterministic synthetic witness through SnarkJS
wtns/prove/verify, checks the prover-returned public signals against the
Keccak-derived expected values, and verifies adversarial witnesses are rejected.
By default it requires a productionReady manifest; --allow-unready-candidate
true is accepted only for testnet candidate evidence refreshes, while
--allow-unready-mainnet-candidate true is the separate explicit opt-in for
mainnet candidate evidence refreshes. Both modes still write the manifest
production blockers into the report. They are evidence only and do not mark
candidate material production-ready. The
attestation-request command emits deterministic unsigned role payloads and
signedPayloadSha256 values for external review/audit/ceremony/rebuild handoff;
roles whose backing transcripts still have blockers are marked
readyForSignature=false. The sign-attestation command signs one ready role body
from a request package with an Ed25519 private key file, derives the public key
fingerprint itself, refuses blocked roles, and writes only public detached
signature material. The attestation-status command is read-only and audits a
request package plus supplied signed role files for request/manifest binding,
transcript blockers, signed body drift, trusted signer verification, and signer
separation before finalization. The finalize-attestations command imports signed role files,
verifies that every signed body exactly matches the request package, enforces
trusted role-separated signatures, and re-materializes the production manifest
from the request package artifacts.
The preflight command is
read-only and fails closed for missing toolchain, partial artifacts, fixture
circuits, malformed verifier material, or manifests that are not productionReady.`;
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
      assertUnsignedCliMaterialize(options);
      return materializeBscGroth16Material(options);
    case "proof-self-test":
    case "proof-test":
    case "prove-self-test":
      return runBscGroth16ProofSelfTest(options);
    case "attestation-request":
    case "attestation-requests":
    case "request-attestations":
      return generateBscGroth16AttestationRequestPackage(options);
    case "sign-attestation":
    case "sign-attestations":
    case "attestation-sign":
      return signBscGroth16AttestationRole(options);
    case "attestation-status":
    case "verify-attestations":
    case "attestation-audit":
      return auditBscGroth16AttestationStatus(options);
    case "finalize-attestations":
    case "finalize-attestation":
    case "attestation-finalize":
      return finalizeBscGroth16Attestations(options);
    case "preflight":
    case "doctor":
      return preflightBscGroth16Material(options);
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
