#!/usr/bin/env node
// Purpose: generate and inspect public BSC SCCP Groth16 circuit/proving
// material without writing operator credentials. Locally generated setup
// output is marked as a production candidate only; production-ready status
// requires externally audited circuit semantics and ceremony evidence.
import { spawn } from "node:child_process";
import { randomBytes, createHash } from "node:crypto";
import {
  copyFile,
  lstat,
  mkdir,
  readFile,
  rename,
  writeFile,
} from "node:fs/promises";
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

const DEFAULT_GENERATED_MATERIAL_OUT =
  "output/sccp-bsc-production/groth16-material";
const PRODUCTION_SNARKJS_R1CS_MIN_BYTES = 64 * 1024;
const PRODUCTION_SNARKJS_ZKEY_MIN_BYTES = 64 * 1024;
const SNARKJS_R1CS_MAGIC = Object.freeze([0x72, 0x31, 0x63, 0x73]);
const SNARKJS_ZKEY_MAGIC = Object.freeze([0x7a, 0x6b, 0x65, 0x79]);

const trim = (value) => String(value ?? "").trim();

function ownValue(record, key) {
  return record && Object.prototype.hasOwnProperty.call(record, key)
    ? record[key]
    : undefined;
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
  return blockers;
}

async function attestationReference(pathName) {
  if (!pathName) {
    return null;
  }
  const resolved = await assertReadableRegularFile(pathName, "attestation");
  return {
    path: repoRelativePath(resolved),
    sha256: await fileSha256(resolved),
  };
}

async function buildAttestationReferences(options) {
  return {
    semanticSccpCircuit: await attestationReference(
      optionalPath(options, ["semantic-attestation", "semantic-sccp-attestation"]),
    ),
    circuitSecurity: await attestationReference(
      optionalPath(options, ["circuit-security-attestation", "circuit-audit"]),
    ),
    trustedSetup: await attestationReference(
      optionalPath(options, ["trusted-setup-attestation", "ceremony-attestation"]),
    ),
    reproducibleBuild: await attestationReference(
      optionalPath(options, ["reproducible-build-attestation"]),
    ),
  };
}

async function artifactRecord(pathName) {
  return {
    path: repoRelativePath(pathName),
    sha256: await fileSha256(pathName),
  };
}

async function snarkjsArtifactBlockers(pathName, label, magic, minBytes) {
  const bytes = await readFile(pathName);
  const blockers = [];
  if (bytes.length < minBytes) {
    blockers.push(`${label} must be at least ${minBytes} bytes for production material`);
  }
  const hasMagic = magic.every((byte, index) => bytes[index] === byte);
  if (!hasMagic) {
    blockers.push(`${label} must start with SnarkJS ${label} magic bytes`);
  }
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
}) {
  const zkeyInitial = join(outDir, "sccp-bsc-signal-binding-v1.0000.zkey");
  const zkeyFinal = join(outDir, "sccp-bsc-signal-binding-v1.final.zkey");
  const snarkjsVerifierKey = join(outDir, "sccp-bsc-signal-binding-v1.snarkjs-verification-key.json");
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
}) {
  const verifierMaterial = snarkjsVerificationKeyToBscVerifierMaterial(
    await readJson(snarkjsVerifierKeyPath, "SnarkJS verification key"),
    { bscNetwork: profile.key },
  );
  const bscVerifierKeyPath = join(outDir, `${profile.key}-bsc-groth16-verifier-key.json`);
  await writePublicJson(bscVerifierKeyPath, verifierMaterial);
  const artifactBlockers = [
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
  const productionBlockers = productionBlockersForMaterial({
    circuitProfile,
    localPtau,
    localPhase2,
    attestations,
  }).concat(artifactBlockers);
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
    artifacts: {
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
    },
    trustedSetup: {
      localPowersOfTau: Boolean(localPtau),
      localPhase2Contribution: Boolean(localPhase2),
      contributionMaterialPersisted: false,
    },
    attestations,
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
  const circuitSourcePath = join(outDir, "sccp-bsc-signal-binding-v1.circom");
  await writePublicText(circuitSourcePath, generateBscSignalBindingCircuitSource());
  await runCommand(circomBin, [
    circuitSourcePath,
    "--r1cs",
    "--wasm",
    "--sym",
    "-o",
    outDir,
  ]);
  const r1csPath = join(outDir, "sccp-bsc-signal-binding-v1.r1cs");
  const wasmPath = join(outDir, "sccp-bsc-signal-binding-v1_js", "sccp-bsc-signal-binding-v1.wasm");
  const symPath = join(outDir, "sccp-bsc-signal-binding-v1.sym");
  const ptauPath = localPtauRequested
    ? await createLocalPtau({ snarkjsBin, outDir, power: createLocalPower })
    : await assertReadableRegularFile(requiredOption(options, "ptau", "Powers of Tau file"), "Powers of Tau file");
  const { zkeyFinal, snarkjsVerifierKey } = await runSnarkjsSetup({
    snarkjsBin,
    r1csPath,
    ptauPath,
    outDir,
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
    circuitProfile: BSC_SIGNAL_BINDING_CIRCUIT_PROFILE,
    attestations,
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
  const circuitProfile = trim(
    ownValue(options, "circuit-profile") ?? BSC_FULL_SCCP_CIRCUIT_PROFILE,
  );
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
  });
}

function usage() {
  return `Usage:
  node scripts/sccp_bsc_groth16_material.mjs generate --bsc-network testnet --ptau <phase2.ptau> [--out-dir ${DEFAULT_GENERATED_MATERIAL_OUT}/testnet] [--circom-bin circom2] [--snarkjs-bin snarkjs]
  node scripts/sccp_bsc_groth16_material.mjs generate --bsc-network testnet --create-local-ptau-power 8 --allow-local-testnet-setup true [--out-dir ${DEFAULT_GENERATED_MATERIAL_OUT}/testnet]
  node scripts/sccp_bsc_groth16_material.mjs materialize --bsc-network testnet|mainnet --r1cs <file.r1cs> --zkey <file.zkey> --snarkjs-verifier-key <verification_key.json> --semantic-attestation <json> --circuit-security-attestation <json> --trusted-setup-attestation <json> --reproducible-build-attestation <json> [--out-dir ${DEFAULT_NATIVE_EVM_PROVER_ARTIFACT_ROOT}/testnet]

The generate command creates real Circom/SnarkJS Groth16 candidate material for
the BSC verifier's 9 public signal words using circuit profile
${BSC_SIGNAL_BINDING_CIRCUIT_PROFILE}. It is not production-ready unless the
full SCCP circuit semantics and ceremony/build evidence are supplied through
materialize.`;
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
