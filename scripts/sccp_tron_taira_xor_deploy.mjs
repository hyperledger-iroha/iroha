#!/usr/bin/env node
// Purpose: compile, sign, deploy, and evidence-check the TRON mainnet
// contracts for the TAIRA XOR SCCP bridge without ever using end-user wallet
// keys. Safe default: no transaction is broadcast unless the command includes
// `--confirm-mainnet taira_tron_xor`.
//
// Prerequisites:
// - Node.js 18+ for built-in fetch.
// - `solc` and `ethers` on NODE_PATH for compile/deploy commands.
// - Optional `TRON_PRO_API_KEY` or `TRON_GRID_API_KEY` for TronGrid calls.
import { createRequire } from "node:module";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { secp256k1 } from "../javascript/iroha_js/node_modules/@noble/curves/secp256k1.js";
import { sha256 } from "../javascript/iroha_js/node_modules/@noble/hashes/sha256.js";
import { keccak_256 } from "../javascript/iroha_js/node_modules/@noble/hashes/sha3.js";
import { compileKotodamaProgram } from "../javascript/iroha_js/src/index.js";

const requireFromScript = createRequire(import.meta.url);
const requireFromCwd = createRequire(`${resolve("noop.js")}`);
const SCRIPT_PATH = fileURLToPath(import.meta.url);
const REPO_ROOT = resolve(dirname(SCRIPT_PATH), "..");

const BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const BASE58_INDEX = new Map(
  Array.from(BASE58_ALPHABET, (character, index) => [character, BigInt(index)]),
);
const DEFAULT_SECRET_OUT = "artifacts/sccp-tron/taira-xor-deployer.secret.json";
const DEFAULT_EVIDENCE_OUT = "artifacts/sccp-tron/taira-xor-deployment.evidence.json";
const DEFAULT_ARTIFACTS_OUT = "artifacts/sccp-tron/contracts";
const DEFAULT_TAIRA_CONTRACT_OUT = "artifacts/sccp-taira/taira-xor-burn-record.contract.json";
const DEFAULT_DEPLOYMENT_OUT = "artifacts/sccp-tron/taira-xor-deployment.plan.json";
const DEFAULT_SIGNED_TRANSACTION_OUT = "artifacts/sccp-tron/signed-transaction.json";
const DEFAULT_BROADCAST_OUT = "artifacts/sccp-tron/broadcast-result.json";
const DEFAULT_TRON_ENDPOINT = "https://api.trongrid.io";
const DEFAULT_DEPLOY_FEE_LIMIT_SUN = 15_000_000_000;
const DEFAULT_TRIGGER_FEE_LIMIT_SUN = 1_000_000_000;
const DEFAULT_ORIGIN_ENERGY_LIMIT = 10_000_000;
const DEFAULT_POLL_ATTEMPTS = 40;
const DEFAULT_POLL_MS = 3_000;
const ROUTE_ID = "taira_tron_xor";
const ASSET_KEY = "xor";
const SCCP_DOMAIN_SORA = 0;
const SCCP_DOMAIN_TRON = 5;
const TRON_MAINNET_NETWORK_ID_HEX =
  `0x${"0".repeat(56)}2b6653dc`;
const CONFIRMATION_TEXT = "taira_tron_xor";
const DEPLOYER_SCHEMA = "iroha-sccp-tron-taira-xor-deployer/v1";
const EVIDENCE_SCHEMA = "iroha-sccp-tron-taira-xor-deployment-evidence/v1";
const SIGNED_TRANSACTION_SCHEMA = "iroha-sccp-tron-signed-transaction/v1";
const DEPLOYMENT_PLAN_SCHEMA = "iroha-sccp-tron-taira-xor-deployment-plan/v1";
const SECP256K1_ORDER =
  0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141n;
const SECP256K1_HALF_ORDER = SECP256K1_ORDER >> 1n;

const textEncoder = new TextEncoder();

const CONTRACT_SOURCES = {
  "contracts/evm/sccp/ISccpMessageVerifier.sol": repoPath(
    "contracts",
    "evm",
    "sccp",
    "ISccpMessageVerifier.sol",
  ),
  "contracts/evm/sccp/Ownable.sol": repoPath("contracts", "evm", "sccp", "Ownable.sol"),
  "contracts/evm/sccp/SccpGroth16Bn254MessageVerifier.sol": repoPath(
    "contracts",
    "evm",
    "sccp",
    "SccpGroth16Bn254MessageVerifier.sol",
  ),
  "contracts/tron/sccp/SccpTronGroth16Bn254MessageVerifier.sol": repoPath(
    "contracts",
    "tron",
    "sccp",
    "SccpTronGroth16Bn254MessageVerifier.sol",
  ),
  "contracts/tron/sccp/SccpTronSourceBridge.sol": repoPath(
    "contracts",
    "tron",
    "sccp",
    "SccpTronSourceBridge.sol",
  ),
  "contracts/tron/sccp/TairaXOR.sol": repoPath("contracts", "tron", "sccp", "TairaXOR.sol"),
  "contracts/tron/sccp/TairaXorSccpBridge.sol": repoPath(
    "contracts",
    "tron",
    "sccp",
    "TairaXorSccpBridge.sol",
  ),
};

const CONTRACT_DEFINITIONS = [
  {
    key: "verifier",
    file: "contracts/tron/sccp/SccpTronGroth16Bn254MessageVerifier.sol",
    contract: "SccpTronGroth16Bn254MessageVerifier",
    deployName: "SccpTronGroth16Bn254MessageVerifier",
  },
  {
    key: "source_bridge",
    file: "contracts/tron/sccp/SccpTronSourceBridge.sol",
    contract: "SccpTronSourceBridge",
    deployName: "SccpTronSourceBridge",
  },
  {
    key: "token",
    file: "contracts/tron/sccp/TairaXOR.sol",
    contract: "TairaXOR",
    deployName: "TairaXOR",
  },
  {
    key: "bridge",
    file: "contracts/tron/sccp/TairaXorSccpBridge.sol",
    contract: "TairaXorSccpBridge",
    deployName: "TairaXorSccpBridge",
  },
];
const TAIRA_BURN_RECORD_CONTRACT_SOURCE = repoPath(
  "contracts",
  "taira",
  "sccp",
  "TairaXorSccpBurnRecord.ko",
);

function repoPath(...segments) {
  return resolve(REPO_ROOT, ...segments);
}

function usage() {
  return `Usage:
  node scripts/sccp_tron_taira_xor_deploy.mjs generate-deployer [--out ${DEFAULT_SECRET_OUT}]
  node scripts/sccp_tron_taira_xor_deploy.mjs account-status [--secret ${DEFAULT_SECRET_OUT}] [--endpoint ${DEFAULT_TRON_ENDPOINT}]
  node scripts/sccp_tron_taira_xor_deploy.mjs compile [--out ${DEFAULT_ARTIFACTS_OUT}]
  node scripts/sccp_tron_taira_xor_deploy.mjs compile-taira-contract [--out ${DEFAULT_TAIRA_CONTRACT_OUT}]
  node scripts/sccp_tron_taira_xor_deploy.mjs deploy --verifier <verifier-key.json> [--secret ${DEFAULT_SECRET_OUT}] [--endpoint ${DEFAULT_TRON_ENDPOINT}] [--out ${DEFAULT_DEPLOYMENT_OUT}] [--broadcast true --confirm-mainnet ${CONFIRMATION_TEXT}]
  node scripts/sccp_tron_taira_xor_deploy.mjs sign-transaction --secret ${DEFAULT_SECRET_OUT} --transaction <unsigned.json> [--out ${DEFAULT_SIGNED_TRANSACTION_OUT}]
  node scripts/sccp_tron_taira_xor_deploy.mjs broadcast --transaction <signed.json> [--endpoint ${DEFAULT_TRON_ENDPOINT}] --confirm-mainnet ${CONFIRMATION_TEXT} [--out ${DEFAULT_BROADCAST_OUT}]
  node scripts/sccp_tron_taira_xor_deploy.mjs evidence --token <addr> --bridge <addr> --source-bridge <addr> --verifier <addr> [--out ${DEFAULT_EVIDENCE_OUT}]
  node scripts/sccp_tron_taira_xor_deploy.mjs self-test

Required optional packages for compile/deploy: solc and ethers. The contract
smoke-test NODE_PATH can be reused, e.g.:
  NODE_PATH=/tmp/iroha-sccp-smoke-node/node_modules node scripts/sccp_tron_taira_xor_deploy.mjs compile

This helper writes only local deployment/deployer artifacts under ignored artifacts/.
End-user TRON wallets must still connect through WalletConnect; never use this deployer for user bridging.`;
}

function bytesToHex(bytes, prefix = true) {
  const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
  return prefix ? `0x${hex}` : hex;
}

function strip0x(value) {
  return String(value).replace(/^0x/iu, "");
}

function assertHexText(value, label, byteLength = null) {
  if (typeof value !== "string") {
    throw new Error(`${label} must be hex text`);
  }
  const hex = strip0x(value);
  if (hex.length === 0 || hex.length % 2 !== 0 || /[^0-9a-f]/iu.test(hex)) {
    throw new Error(`${label} must be even-length hex`);
  }
  if (byteLength !== null && hex.length !== byteLength * 2) {
    throw new Error(`${label} must be ${byteLength} bytes`);
  }
  return hex.toLowerCase();
}

function hexToBytes(value, label, byteLength = null) {
  const hex = assertHexText(value, label, byteLength);
  return Uint8Array.from(hex.match(/.{1,2}/gu).map((byte) => Number.parseInt(byte, 16)));
}

function bytesToBigInt(bytes) {
  return BigInt(`0x${bytesToHex(bytes, false) || "0"}`);
}

function normalizeBytes32(value, label) {
  const bytes = hexToBytes(value, label, 32);
  if (bytes.every((byte) => byte === 0)) {
    throw new Error(`${label} must be non-zero bytes32`);
  }
  return bytesToHex(bytes);
}

function normalizeUint32(value, label) {
  if (typeof value === "string" && /^0x[0-9a-f]+$/iu.test(value)) {
    const parsed = Number.parseInt(value.slice(2), 16);
    if (Number.isInteger(parsed) && parsed >= 0 && parsed <= 0xffffffff) return parsed;
  }
  const parsed = typeof value === "bigint" ? value : BigInt(String(value));
  if (parsed < 0n || parsed > 0xffffffffn) {
    throw new Error(`${label} must fit uint32`);
  }
  return Number(parsed);
}

function normalizeSun(value, label, fallback) {
  const source = value === undefined || value === null || value === "" ? fallback : value;
  const parsed = Number(source);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new Error(`${label} must be a positive safe integer SUN amount`);
  }
  return parsed;
}

function normalizeUint256(value, label) {
  if (typeof value === "number" && !Number.isSafeInteger(value)) {
    throw new Error(`${label} must be a safe integer, decimal string, or hex string`);
  }
  const parsed =
    typeof value === "bigint"
      ? value
      : typeof value === "string" && /^0x[0-9a-f]+$/iu.test(value)
        ? BigInt(value)
        : BigInt(String(value));
  if (parsed < 0n || parsed >= (1n << 256n)) {
    throw new Error(`${label} must fit uint256`);
  }
  return parsed.toString(10);
}

function base58Encode(bytes) {
  let value = BigInt(`0x${bytesToHex(bytes, false) || "0"}`);
  let encoded = "";
  while (value > 0n) {
    const remainder = Number(value % 58n);
    encoded = BASE58_ALPHABET[remainder] + encoded;
    value /= 58n;
  }
  for (const byte of bytes) {
    if (byte !== 0) break;
    encoded = `${BASE58_ALPHABET[0]}${encoded}`;
  }
  return encoded || BASE58_ALPHABET[0];
}

function base58Decode(value) {
  let numeric = 0n;
  for (const character of value) {
    const digit = BASE58_INDEX.get(character);
    if (digit === undefined) {
      throw new Error("TRON address must use canonical Base58Check characters");
    }
    numeric = numeric * 58n + digit;
  }
  let payload = new Uint8Array();
  if (numeric !== 0n) {
    const hex = numeric.toString(16);
    const normalizedHex = hex.length % 2 === 0 ? hex : `0${hex}`;
    payload = Uint8Array.from(
      normalizedHex.match(/.{1,2}/gu).map((byte) => Number.parseInt(byte, 16)),
    );
  }
  let leadingZeroes = 0;
  while (leadingZeroes < value.length && value[leadingZeroes] === BASE58_ALPHABET[0]) {
    leadingZeroes += 1;
  }
  if (leadingZeroes === 0) return payload;
  const out = new Uint8Array(leadingZeroes + payload.length);
  out.set(payload, leadingZeroes);
  return out;
}

function tronBase58Check(payload) {
  const checksum = sha256(sha256(payload)).slice(0, 4);
  return base58Encode(new Uint8Array([...payload, ...checksum]));
}

function normalizeTronAddress(value, label) {
  if (typeof value !== "string" || value.trim() !== value || value.length === 0) {
    throw new Error(`${label} must be a canonical TRON address`);
  }
  if (value.startsWith("T")) {
    const decoded = base58Decode(value);
    if (decoded.length !== 25) {
      throw new Error(`${label} must decode to 25 Base58Check bytes`);
    }
    const payload = decoded.slice(0, 21);
    const checksum = decoded.slice(21);
    const expected = sha256(sha256(payload)).slice(0, 4);
    if (!checksum.every((byte, index) => byte === expected[index])) {
      throw new Error(`${label} checksum is invalid`);
    }
    if (payload[0] !== 0x41 || payload.slice(1).every((byte) => byte === 0)) {
      throw new Error(`${label} must be a non-zero TRON mainnet address`);
    }
    if (tronBase58Check(payload) !== value) {
      throw new Error(`${label} must be canonical Base58Check`);
    }
    return {
      payload,
      base58: value,
      hex: bytesToHex(payload),
      solidity: bytesToHex(payload.slice(1)),
    };
  }

  const hex = strip0x(value);
  const payload =
    hex.length === 40
      ? new Uint8Array([0x41, ...hexToBytes(hex, label, 20)])
      : hexToBytes(hex, label, 21);
  if (payload[0] !== 0x41 || payload.slice(1).every((byte) => byte === 0)) {
    throw new Error(`${label} must be a non-zero TRON mainnet address`);
  }
  return {
    payload,
    base58: tronBase58Check(payload),
    hex: bytesToHex(payload),
    solidity: bytesToHex(payload.slice(1)),
  };
}

function normalizeTronBase58Address(value, label) {
  if (typeof value !== "string" || !value.startsWith("T")) {
    throw new Error(`${label} must be a canonical TRON Base58Check mainnet address`);
  }
  return normalizeTronAddress(value, label);
}

function routeHash(text) {
  return bytesToHex(keccak_256(textEncoder.encode(text)));
}

function tronAddressFromPublicKey(publicKey) {
  const evmAddress = keccak_256(publicKey.slice(1)).slice(-20);
  const payload = new Uint8Array(21);
  payload[0] = 0x41;
  payload.set(evmAddress, 1);
  return {
    payload,
    hex: bytesToHex(payload),
    base58: tronBase58Check(payload),
    solidity: bytesToHex(payload.slice(1)),
  };
}

function tronAddressFromPrivateKey(privateKey) {
  return tronAddressFromPublicKey(secp256k1.getPublicKey(privateKey, false));
}

function parseArgs(argv) {
  const [command, ...rest] = argv;
  const options = {};
  for (let index = 0; index < rest.length; index += 1) {
    const entry = rest[index];
    if (!entry.startsWith("--")) {
      throw new Error(`Unexpected argument: ${entry}`);
    }
    const key = entry.slice(2);
    const value = rest[index + 1];
    if (!value || value.startsWith("--")) {
      throw new Error(`Missing value for --${key}`);
    }
    options[key] = value;
    index += 1;
  }
  return { command, options };
}

async function writeJson(path, value, mode = 0o600) {
  const out = resolve(path);
  await mkdir(dirname(out), { recursive: true });
  await writeFile(`${out}.tmp`, `${JSON.stringify(value, null, 2)}\n`, { mode });
  await rename(`${out}.tmp`, out);
  return out;
}

async function readJson(path, label) {
  let text;
  try {
    text = await readFile(resolve(path), "utf8");
  } catch (error) {
    throw new Error(`${label} could not be read: ${error.message}`);
  }
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`${label} is not valid JSON: ${error.message}`);
  }
}

function loadNodeModule(name, purpose) {
  try {
    return requireFromScript(name);
  } catch (scriptError) {
    try {
      return requireFromCwd(name);
    } catch {
      throw new Error(
        `${purpose} requires the optional "${name}" package. Install it or set NODE_PATH ` +
          `to a node_modules directory that contains "${name}". Original error: ${scriptError.message}`,
      );
    }
  }
}

function parsePrivateKeyHex(value, label = "private_key_hex") {
  const privateKey = hexToBytes(value, label, 32);
  if (!secp256k1.utils.isValidPrivateKey(privateKey)) {
    throw new Error(`${label} is not a valid secp256k1 private key`);
  }
  return privateKey;
}

async function loadDeployerSecret(path) {
  const secret = await readJson(path, "deployer secret");
  if (secret.schema !== DEPLOYER_SCHEMA) {
    throw new Error(`deployer secret schema must be ${DEPLOYER_SCHEMA}`);
  }
  const privateKey = parsePrivateKeyHex(secret.private_key_hex);
  const derived = tronAddressFromPrivateKey(privateKey);
  if (secret.address_base58 !== derived.base58) {
    throw new Error("deployer secret address_base58 does not match private_key_hex");
  }
  if (secret.address_hex !== derived.hex) {
    throw new Error("deployer secret address_hex does not match private_key_hex");
  }
  return {
    privateKey,
    privateKeyHex: bytesToHex(privateKey, false),
    address: derived,
  };
}

async function generateDeployer(options) {
  const privateKey = secp256k1.utils.randomPrivateKey();
  const address = tronAddressFromPrivateKey(privateKey);
  const createdAt = new Date().toISOString();
  const out = await writeJson(options.out ?? DEFAULT_SECRET_OUT, {
    schema: DEPLOYER_SCHEMA,
    created_at: createdAt,
    network: "tron-mainnet",
    address_base58: address.base58,
    address_hex: address.hex,
    private_key_hex: bytesToHex(privateKey, false),
    warning:
      "Deployment account only. Fund with the minimum required TRX/energy, deploy, record evidence, then rotate remaining funds out. Do not use for end-user bridging.",
  });
  console.log(JSON.stringify({
    wrote: out,
    address_base58: address.base58,
    address_hex: address.hex,
    next_step: "Fund this deployer with TRX/energy before broadcasting deployment transactions.",
  }, null, 2));
}

async function compileTronContracts(options = {}) {
  const solc = loadNodeModule("solc", "TRON SCCP contract compilation");
  const sources = {};
  for (const [sourceName, path] of Object.entries(CONTRACT_SOURCES)) {
    sources[sourceName] = { content: await readFile(path, "utf8") };
  }
  const input = {
    language: "Solidity",
    sources,
    settings: {
      optimizer: { enabled: true, runs: 200 },
      outputSelection: {
        "*": {
          "*": ["abi", "evm.bytecode.object", "evm.deployedBytecode.object"],
        },
      },
    },
  };
  const output = JSON.parse(solc.compile(JSON.stringify(input)));
  const errors = output.errors ?? [];
  const fatal = errors.filter((entry) => entry.severity === "error");
  if (fatal.length > 0) {
    throw new Error(fatal.map((entry) => entry.formattedMessage).join("\n"));
  }

  const artifacts = {};
  for (const definition of CONTRACT_DEFINITIONS) {
    const contract = output.contracts?.[definition.file]?.[definition.contract];
    if (!contract?.evm?.bytecode?.object) {
      throw new Error(`${definition.contract} did not produce bytecode`);
    }
    artifacts[definition.key] = {
      contractName: definition.contract,
      sourceName: definition.file,
      abi: contract.abi,
      bytecode: `0x${contract.evm.bytecode.object}`,
      deployedBytecode: `0x${contract.evm.deployedBytecode.object}`,
      bytecodeSha256: bytesToHex(sha256(hexToBytes(contract.evm.bytecode.object, `${definition.contract} bytecode`))),
      deployedBytecodeSha256: bytesToHex(
        sha256(hexToBytes(contract.evm.deployedBytecode.object, `${definition.contract} runtime bytecode`)),
      ),
    };
  }

  if (options.out) {
    const outDir = resolve(options.out);
    await mkdir(outDir, { recursive: true });
    for (const [key, artifact] of Object.entries(artifacts)) {
      await writeJson(resolve(outDir, `${key}.json`), artifact, 0o644);
    }
    await writeJson(
      resolve(outDir, "manifest.json"),
      {
        schema: "iroha-sccp-tron-contract-artifacts/v1",
        created_at: new Date().toISOString(),
        solc_version: typeof solc.version === "function" ? solc.version() : "unknown",
        contracts: Object.fromEntries(
          Object.entries(artifacts).map(([key, artifact]) => [
            key,
            {
              contractName: artifact.contractName,
              sourceName: artifact.sourceName,
              bytecodeSha256: artifact.bytecodeSha256,
              deployedBytecodeSha256: artifact.deployedBytecodeSha256,
            },
          ]),
        ),
      },
      0o644,
    );
  }
  return {
    artifacts,
    warnings: errors.filter((entry) => entry.severity !== "error"),
    solcVersion: typeof solc.version === "function" ? solc.version() : "unknown",
  };
}

async function compileTairaBurnRecordContract(options = {}) {
  const source = await readFile(TAIRA_BURN_RECORD_CONTRACT_SOURCE, "utf8");
  const sourceName = "contracts/taira/sccp/TairaXorSccpBurnRecord.ko";
  const compiled = compileKotodamaProgram(source, {
    sourceName,
    forceZk: true,
  });
  if (compiled.diagnostics.length > 0) {
    throw new Error(
      compiled.diagnostics
        .map((entry) => `${entry.severity}: ${entry.message}`)
        .join("\n"),
    );
  }
  if (compiled.artifactBytes.length === 0 || compiled.artifactBytes[6] !== 1) {
    throw new Error("TAIRA burn-record contract must compile with IVM ZK mode bit");
  }

  const artifactBytes = Uint8Array.from(compiled.artifactBytes);
  const contract = {
    schema: "iroha-sccp-taira-xor-burn-record-contract/v1",
    created_at: new Date().toISOString(),
    route_id: ROUTE_ID,
    asset_key: ASSET_KEY,
    source_name: sourceName,
    compiler_fingerprint: compiled.compilerFingerprint,
    code_hash: compiled.codeHashHex,
    abi_hash: compiled.abiHashHex,
    artifact_sha256: bytesToHex(sha256(artifactBytes)),
    artifact_b64: Buffer.from(artifactBytes).toString("base64"),
    manifest: compiled.manifest,
    execution: {
      executable: "IvmProved",
      force_zk_mode: true,
      entrypoint: "burn_and_record",
      settlement_instruction: "Burn<Numeric, Asset>",
      record_instruction: "RecordSccpMessage",
    },
  };
  if (options.out) {
    await writeJson(options.out, contract, 0o644);
  }
  return contract;
}

async function compileTairaContractCommand(options) {
  const out = options.out ?? DEFAULT_TAIRA_CONTRACT_OUT;
  const contract = await compileTairaBurnRecordContract({ out });
  console.log(JSON.stringify({
    wrote: resolve(out),
    route_id: contract.route_id,
    code_hash: contract.code_hash,
    artifact_sha256: contract.artifact_sha256,
    entrypoint: contract.execution.entrypoint,
  }, null, 2));
}

async function compileCommand(options) {
  const { artifacts, warnings, solcVersion } = await compileTronContracts({
    out: options.out ?? DEFAULT_ARTIFACTS_OUT,
  });
  console.log(JSON.stringify({
    wrote: resolve(options.out ?? DEFAULT_ARTIFACTS_OUT),
    solc_version: solcVersion,
    warnings: warnings.map((entry) => entry.formattedMessage ?? entry.message),
    contracts: Object.fromEntries(
      Object.entries(artifacts).map(([key, artifact]) => [
        key,
        {
          contractName: artifact.contractName,
          bytecodeSha256: artifact.bytecodeSha256,
          deployedBytecodeSha256: artifact.deployedBytecodeSha256,
        },
      ]),
    ),
  }, null, 2));
}

function pickField(record, names, label) {
  for (const name of names) {
    if (record[name] !== undefined) return record[name];
  }
  throw new Error(`verifier material is missing ${label}`);
}

function flattenArray(value, label) {
  if (!Array.isArray(value)) {
    throw new Error(`${label} must be an array`);
  }
  return value.flat(Infinity);
}

function normalizeUint256Array(value, label, expectedLength = null) {
  const values = flattenArray(value, label).map((entry, index) =>
    normalizeUint256(entry, `${label}[${index}]`),
  );
  if (expectedLength !== null && values.length !== expectedLength) {
    throw new Error(`${label} must contain ${expectedLength} uint256 values`);
  }
  return values;
}

function normalizeVerifierConstructorArgs(material, options = {}) {
  if (!material || typeof material !== "object" || Array.isArray(material)) {
    throw new Error("verifier material must be a JSON object");
  }
  const alpha1 = normalizeUint256Array(
    pickField(material, ["alpha1", "configuredAlpha1", "vk_alpha_1"], "alpha1"),
    "alpha1",
    2,
  );
  const beta2 = normalizeUint256Array(
    pickField(material, ["beta2", "configuredBeta2", "vk_beta_2"], "beta2"),
    "beta2",
    4,
  );
  const gamma2 = normalizeUint256Array(
    pickField(material, ["gamma2", "configuredGamma2", "vk_gamma_2"], "gamma2"),
    "gamma2",
    4,
  );
  const delta2 = normalizeUint256Array(
    pickField(material, ["delta2", "configuredDelta2", "vk_delta_2"], "delta2"),
    "delta2",
    4,
  );
  const ic = normalizeUint256Array(
    pickField(material, ["ic", "configuredIc", "vk_ic", "IC"], "ic"),
    "ic",
  );
  if (ic.length < 2) {
    throw new Error("ic must contain at least two points");
  }
  const expectedVerifierKeyHash = normalizeBytes32(
    options.expectedVerifierKeyHash ??
      material.expectedVerifierKeyHash ??
      material.verifierKeyHash ??
      material.verifyingKeyHash,
    "expectedVerifierKeyHash",
  );
  const proofFamily = String(options.proofFamily ?? material.proofFamily ?? "stark-fri-v1");
  if (proofFamily !== "stark-fri-v1") {
    throw new Error("proofFamily must be stark-fri-v1 for production TRON SCCP");
  }
  const networkId = normalizeBytes32(
    options.networkId ?? material.networkId ?? TRON_MAINNET_NETWORK_ID_HEX,
    "networkId",
  );
  if (networkId !== TRON_MAINNET_NETWORK_ID_HEX) {
    throw new Error("networkId must be TRON mainnet for taira_tron_xor deployment");
  }
  const sourceDomain = normalizeUint32(
    options.sourceDomain ?? material.sourceDomain ?? SCCP_DOMAIN_SORA,
    "sourceDomain",
  );
  const targetDomain = normalizeUint32(
    options.targetDomain ?? material.targetDomain ?? SCCP_DOMAIN_TRON,
    "targetDomain",
  );
  if (sourceDomain !== SCCP_DOMAIN_SORA || targetDomain !== SCCP_DOMAIN_TRON) {
    throw new Error("destination verifier domains must be SORA -> TRON");
  }
  return [
    alpha1,
    beta2,
    gamma2,
    delta2,
    ic,
    expectedVerifierKeyHash,
    proofFamily,
    networkId,
    sourceDomain,
    targetDomain,
  ];
}

function decodeTronErrorMessage(value) {
  if (typeof value !== "string") return "";
  const hex = strip0x(value);
  if (hex.length > 0 && hex.length % 2 === 0 && !/[^0-9a-f]/iu.test(hex)) {
    try {
      return new TextDecoder().decode(hexToBytes(hex, "TRON error message"));
    } catch {
      return value;
    }
  }
  return value;
}

function endpointUrl(endpoint, path) {
  return `${String(endpoint).replace(/\/+$/u, "")}/${path.replace(/^\/+/u, "")}`;
}

function runtimeApiKey(options) {
  return options["api-key"] ?? process.env.TRON_PRO_API_KEY ?? process.env.TRON_GRID_API_KEY ?? "";
}

async function tronPost(endpoint, path, body, options = {}) {
  const headers = {
    accept: "application/json",
    "content-type": "application/json",
  };
  const apiKey = runtimeApiKey(options);
  if (apiKey) headers["TRON-PRO-API-KEY"] = apiKey;
  const response = await fetch(endpointUrl(endpoint, path), {
    method: "POST",
    headers,
    body: JSON.stringify(body),
  });
  const text = await response.text();
  let payload;
  try {
    payload = text ? JSON.parse(text) : {};
  } catch (error) {
    throw new Error(`TRON ${path} returned non-JSON response: ${error.message}`);
  }
  if (!response.ok) {
    throw new Error(`TRON ${path} failed with HTTP ${response.status}: ${text}`);
  }
  return payload;
}

function assertTronResult(payload, context) {
  const result = payload?.result;
  if (result === false || (result && typeof result === "object" && result.result === false)) {
    const message = decodeTronErrorMessage(result?.message ?? payload.message ?? payload.code);
    throw new Error(`${context} failed${message ? `: ${message}` : ""}`);
  }
  if (payload?.Error) {
    throw new Error(`${context} failed: ${payload.Error}`);
  }
}

function extractTransaction(payload, label = "TRON response") {
  const transaction = payload?.transaction?.raw_data_hex
    ? payload.transaction
    : payload?.signed_transaction?.raw_data_hex
      ? payload.signed_transaction
      : payload?.transaction?.transaction?.raw_data_hex
        ? payload.transaction.transaction
        : payload?.raw_data_hex
          ? payload
          : null;
  if (!transaction || typeof transaction !== "object") {
    throw new Error(`${label} does not contain a transaction with raw_data_hex`);
  }
  return transaction;
}

function transactionRawDataHash(transaction, label = "transaction") {
  const rawData = hexToBytes(transaction.raw_data_hex, `${label}.raw_data_hex`);
  if (rawData.length === 0) {
    throw new Error(`${label}.raw_data_hex must not be empty`);
  }
  const txid = bytesToHex(sha256(rawData), false);
  if (transaction.txID !== undefined && String(transaction.txID).toLowerCase() !== txid) {
    throw new Error(`${label}.txID does not match SHA-256(raw_data_hex)`);
  }
  return { rawData, hash: sha256(rawData), txid };
}

function transactionOwnerAddress(transaction, label = "transaction") {
  const contracts = transaction.raw_data?.contract;
  if (!Array.isArray(contracts) || contracts.length !== 1) {
    throw new Error(`${label} must contain exactly one raw_data.contract entry`);
  }
  const value = contracts[0]?.parameter?.value;
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} contract parameter value is missing`);
  }
  const ownerAddress = value.owner_address ?? value.ownerAddress;
  if (typeof ownerAddress !== "string") {
    throw new Error(`${label} contract owner_address is missing`);
  }
  const normalizedOwner = normalizeTronAddress(ownerAddress, `${label}.owner_address`);
  const originAddress = value.new_contract?.origin_address ?? value.newContract?.origin_address;
  if (typeof originAddress === "string") {
    const normalizedOrigin = normalizeTronAddress(originAddress, `${label}.new_contract.origin_address`);
    if (normalizedOrigin.base58 !== normalizedOwner.base58) {
      throw new Error(`${label} deployment origin_address does not match owner_address`);
    }
  }
  return normalizedOwner;
}

function assertTransactionOwner(transaction, expected, label = "transaction") {
  const owner = transactionOwnerAddress(transaction, label);
  if (owner.base58 !== expected.base58) {
    throw new Error(`${label} owner ${owner.base58} does not match deployer ${expected.base58}`);
  }
  return owner;
}

function tronRecoverableSignatureIsCanonical(signature) {
  if (signature.length !== 65) return false;
  const recoveryId = signature[64];
  if (!((recoveryId >= 0 && recoveryId <= 3) || (recoveryId >= 27 && recoveryId <= 30))) {
    return false;
  }
  const r = bytesToBigInt(signature.slice(0, 32));
  const s = bytesToBigInt(signature.slice(32, 64));
  return r > 0n && r < SECP256K1_ORDER && s > 0n && s <= SECP256K1_HALF_ORDER;
}

function signTransactionPayload(transaction, deployer) {
  if (Array.isArray(transaction.signature) && transaction.signature.length > 0) {
    throw new Error("transaction is already signed");
  }
  assertTransactionOwner(transaction, deployer.address);
  const { hash, txid } = transactionRawDataHash(transaction);
  const signatureObject = secp256k1.sign(hash, deployer.privateKey, {
    prehash: false,
    lowS: true,
  });
  const compact = signatureObject.toCompactRawBytes();
  const signature = new Uint8Array(65);
  signature.set(compact);
  signature[64] = signatureObject.recovery;
  if (!tronRecoverableSignatureIsCanonical(signature)) {
    throw new Error("generated TRON signature is not canonical");
  }
  const recovered = tronAddressFromPublicKey(
    signatureObject.recoverPublicKey(hash).toRawBytes(false),
  );
  if (recovered.base58 !== deployer.address.base58) {
    throw new Error("generated TRON signature does not recover to deployer");
  }
  const signed = JSON.parse(JSON.stringify(transaction));
  signed.txID = txid;
  signed.signature = [bytesToHex(signature, false)];
  return {
    signed,
    metadata: {
      txid,
      signature: bytesToHex(signature, false),
      signature_recovery_id: signature[64],
      signature_recovered_address: recovered.hex,
      signature_recovered_base58: recovered.base58,
      signature_recovers_to_owner: true,
      owner_address: deployer.address.hex,
      owner_base58: deployer.address.base58,
    },
  };
}

async function signTransactionCommand(options) {
  if (!options.transaction) throw new Error("--transaction is required");
  const deployer = await loadDeployerSecret(options.secret ?? DEFAULT_SECRET_OUT);
  const payload = await readJson(options.transaction, "unsigned transaction");
  const transaction = extractTransaction(payload, "unsigned transaction");
  const signed = signTransactionPayload(transaction, deployer);
  const out = await writeJson(options.out ?? DEFAULT_SIGNED_TRANSACTION_OUT, {
    schema: SIGNED_TRANSACTION_SCHEMA,
    signed_at: new Date().toISOString(),
    ...signed.metadata,
    transaction: signed.signed,
  });
  console.log(JSON.stringify({ wrote: out, ...signed.metadata }, null, 2));
}

function requireMainnetConfirmation(options, action) {
  if (options["confirm-mainnet"] !== CONFIRMATION_TEXT) {
    throw new Error(
      `${action} targets TRON mainnet and requires --confirm-mainnet ${CONFIRMATION_TEXT}`,
    );
  }
}

async function broadcastSignedTransaction(endpoint, transaction, options = {}) {
  const payload = await tronPost(endpoint, "wallet/broadcasttransaction", transaction, options);
  assertTronResult(payload, "broadcasttransaction");
  if (payload.result !== true && payload.result?.result !== true) {
    throw new Error(`broadcasttransaction did not report success: ${JSON.stringify(payload)}`);
  }
  return payload;
}

async function broadcastCommand(options) {
  if (!options.transaction) throw new Error("--transaction is required");
  requireMainnetConfirmation(options, "broadcast");
  const payload = await readJson(options.transaction, "signed transaction");
  const transaction = extractTransaction(payload, "signed transaction");
  if (!Array.isArray(transaction.signature) || transaction.signature.length !== 1) {
    throw new Error("signed transaction must contain exactly one signature");
  }
  transactionRawDataHash(transaction, "signed transaction");
  const result = await broadcastSignedTransaction(
    options.endpoint ?? DEFAULT_TRON_ENDPOINT,
    transaction,
    options,
  );
  const out = await writeJson(options.out ?? DEFAULT_BROADCAST_OUT, {
    schema: "iroha-sccp-tron-broadcast-result/v1",
    broadcast_at: new Date().toISOString(),
    txid: transaction.txID,
    endpoint: options.endpoint ?? DEFAULT_TRON_ENDPOINT,
    result,
  });
  console.log(JSON.stringify({ wrote: out, txid: transaction.txID, result }, null, 2));
}

function sleep(ms) {
  return new Promise((resolveSleep) => {
    setTimeout(resolveSleep, ms);
  });
}

async function waitForTransactionInfo(endpoint, txid, options = {}) {
  const attempts = Number(options["poll-attempts"] ?? DEFAULT_POLL_ATTEMPTS);
  const pollMs = Number(options["poll-ms"] ?? DEFAULT_POLL_MS);
  if (!Number.isInteger(attempts) || attempts <= 0) {
    throw new Error("--poll-attempts must be a positive integer");
  }
  if (!Number.isInteger(pollMs) || pollMs <= 0) {
    throw new Error("--poll-ms must be a positive integer");
  }
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    const info = await tronPost(endpoint, "walletsolidity/gettransactioninfobyid", { value: txid }, options);
    if (info && Object.keys(info).length > 0) {
      const receiptResult = info.receipt?.result ?? info.result;
      if (receiptResult && receiptResult !== "SUCCESS") {
        throw new Error(`transaction ${txid} finalized with result ${receiptResult}`);
      }
      return info;
    }
    await sleep(pollMs);
  }
  throw new Error(`transaction ${txid} was not found after ${attempts} polling attempts`);
}

function extractDeployAddress(response, label) {
  const candidates = [
    response.contract_address,
    response.contractAddress,
    response.transaction?.contract_address,
    response.transaction?.contractAddress,
    response.transaction?.raw_data?.contract?.[0]?.parameter?.value?.new_contract?.contract_address,
    response.transaction?.raw_data?.contract?.[0]?.parameter?.value?.newContract?.contract_address,
  ].filter((value) => typeof value === "string" && value.length > 0);
  if (candidates.length === 0) {
    throw new Error(`${label} deployment response did not include a contract address`);
  }
  return normalizeTronAddress(candidates[0], `${label} contract_address`);
}

function deployRequest(ethers, deployer, artifact, constructorArgs, options, contractName) {
  const iface = new ethers.Interface(artifact.abi);
  const parameter = strip0x(iface.encodeDeploy(constructorArgs));
  return {
    owner_address: deployer.address.base58,
    abi: JSON.stringify(artifact.abi),
    bytecode: strip0x(artifact.bytecode),
    fee_limit: normalizeSun(options["fee-limit"], "--fee-limit", DEFAULT_DEPLOY_FEE_LIMIT_SUN),
    parameter,
    origin_energy_limit: normalizeSun(
      options["origin-energy-limit"],
      "--origin-energy-limit",
      DEFAULT_ORIGIN_ENERGY_LIMIT,
    ),
    name: contractName,
    call_value: 0,
    consume_user_resource_percent: 100,
    visible: true,
  };
}

function triggerRequest(ethers, deployer, artifact, contractAddress, functionName, args, options) {
  const iface = new ethers.Interface(artifact.abi);
  return {
    owner_address: deployer.address.base58,
    contract_address: contractAddress.base58,
    data: strip0x(iface.encodeFunctionData(functionName, args)),
    fee_limit: normalizeSun(options["trigger-fee-limit"], "--trigger-fee-limit", DEFAULT_TRIGGER_FEE_LIMIT_SUN),
    call_value: 0,
    visible: true,
  };
}

async function submitSignedStep(endpoint, transaction, deployer, step, options) {
  const signed = signTransactionPayload(transaction, deployer);
  const broadcast = await broadcastSignedTransaction(endpoint, signed.signed, options);
  const receipt = await waitForTransactionInfo(endpoint, signed.metadata.txid, options);
  return {
    ...step,
    txid: signed.metadata.txid,
    signed: signed.metadata,
    broadcast,
    receipt,
  };
}

async function createDeployStep(context, key, artifact, constructorArgs) {
  const definition = CONTRACT_DEFINITIONS.find((entry) => entry.key === key);
  const request = deployRequest(
    context.ethers,
    context.deployer,
    artifact,
    constructorArgs,
    context.options,
    definition.deployName,
  );
  const response = await tronPost(context.endpoint, "wallet/deploycontract", request, context.options);
  assertTronResult(response, `${definition.deployName} deploycontract`);
  const transaction = extractTransaction(response, `${definition.deployName} deploycontract`);
  transactionRawDataHash(transaction, `${definition.deployName} deploy transaction`);
  assertTransactionOwner(transaction, context.deployer.address, `${definition.deployName} deploy transaction`);
  const address = extractDeployAddress(response, definition.deployName);
  const step = {
    kind: "deploy",
    key,
    contractName: definition.contract,
    address_base58: address.base58,
    address_hex: address.hex,
    address_solidity: address.solidity,
    request,
    unsigned_response: response,
    unsigned_transaction: transaction,
  };
  if (!context.broadcast) return step;
  return submitSignedStep(context.endpoint, transaction, context.deployer, step, context.options);
}

async function createTriggerStep(context, key, artifact, contractAddress, functionName, args) {
  const request = triggerRequest(
    context.ethers,
    context.deployer,
    artifact,
    contractAddress,
    functionName,
    args,
    context.options,
  );
  const response = await tronPost(context.endpoint, "wallet/triggersmartcontract", request, context.options);
  assertTronResult(response, `${functionName} triggersmartcontract`);
  const transaction = extractTransaction(response, `${functionName} triggersmartcontract`);
  transactionRawDataHash(transaction, `${functionName} transaction`);
  assertTransactionOwner(transaction, context.deployer.address, `${functionName} transaction`);
  const step = {
    kind: "trigger",
    key,
    contract: contractAddress.base58,
    functionName,
    args,
    request,
    unsigned_response: response,
    unsigned_transaction: transaction,
  };
  if (!context.broadcast) return step;
  return submitSignedStep(context.endpoint, transaction, context.deployer, step, context.options);
}

async function deployCommand(options) {
  if (!options.verifier) throw new Error("--verifier is required");
  const broadcast = options.broadcast === "true";
  if (options.broadcast !== undefined && !["true", "false"].includes(options.broadcast)) {
    throw new Error("--broadcast must be true or false");
  }
  if (broadcast) requireMainnetConfirmation(options, "deploy");
  const ethersModule = loadNodeModule("ethers", "TRON SCCP deployment ABI encoding");
  const ethers = ethersModule.ethers ?? ethersModule;
  const deployer = await loadDeployerSecret(options.secret ?? DEFAULT_SECRET_OUT);
  const verifierMaterial = await readJson(options.verifier, "verifier material");
  const verifierArgs = normalizeVerifierConstructorArgs(verifierMaterial, options);
  const { artifacts, solcVersion } = await compileTronContracts(
    options["artifacts-out"] ? { out: options["artifacts-out"] } : {},
  );
  const endpoint = options.endpoint ?? DEFAULT_TRON_ENDPOINT;
  const context = { endpoint, deployer, ethers, options, broadcast };
  const steps = [];

  const verifierStep = await createDeployStep(context, "verifier", artifacts.verifier, verifierArgs);
  steps.push(verifierStep);
  const verifierAddress = normalizeTronAddress(verifierStep.address_base58, "verifier address");

  const sourceBridgeArgs = [TRON_MAINNET_NETWORK_ID_HEX, SCCP_DOMAIN_TRON, SCCP_DOMAIN_SORA];
  const sourceBridgeStep = await createDeployStep(
    context,
    "source_bridge",
    artifacts.source_bridge,
    sourceBridgeArgs,
  );
  steps.push(sourceBridgeStep);
  const sourceBridgeAddress = normalizeTronAddress(
    sourceBridgeStep.address_base58,
    "source bridge address",
  );

  const tokenStep = await createDeployStep(context, "token", artifacts.token, []);
  steps.push(tokenStep);
  const tokenAddress = normalizeTronAddress(tokenStep.address_base58, "token address");

  const bridgeArgs = [
    tokenAddress.solidity,
    verifierAddress.solidity,
    sourceBridgeAddress.solidity,
    routeHash(ROUTE_ID),
    routeHash(ASSET_KEY),
  ];
  const bridgeStep = await createDeployStep(context, "bridge", artifacts.bridge, bridgeArgs);
  steps.push(bridgeStep);
  const bridgeAddress = normalizeTronAddress(bridgeStep.address_base58, "bridge address");

  if (broadcast) {
    steps.push(
      await createTriggerStep(
        context,
        "token_set_bridge",
        artifacts.token,
        tokenAddress,
        "setBridge",
        [bridgeAddress.solidity],
      ),
    );
    steps.push(
      await createTriggerStep(context, "token_lock_bridge", artifacts.token, tokenAddress, "lockBridge", []),
    );
    steps.push(
      await createTriggerStep(
        context,
        "source_bridge_transfer_ownership",
        artifacts.source_bridge,
        sourceBridgeAddress,
        "transferOwnership",
        [bridgeAddress.solidity],
      ),
    );
    steps.push(
      await createTriggerStep(
        context,
        "verifier_emit_destination_binding",
        artifacts.verifier,
        verifierAddress,
        "emitDestinationBindingConfigured",
        [],
      ),
    );
  }

  const plan = {
    schema: DEPLOYMENT_PLAN_SCHEMA,
    created_at: new Date().toISOString(),
    endpoint,
    network: "tron-mainnet",
    network_id_hex: TRON_MAINNET_NETWORK_ID_HEX,
    route_id: ROUTE_ID,
    route_id_hash: routeHash(ROUTE_ID),
    asset_key: ASSET_KEY,
    asset_key_hash: routeHash(ASSET_KEY),
    broadcast,
    solc_version: solcVersion,
    deployer_address_base58: deployer.address.base58,
    deployer_address_hex: deployer.address.hex,
    deployment_addresses: {
      verifier: verifierAddress.base58,
      source_bridge: sourceBridgeAddress.base58,
      token: tokenAddress.base58,
      bridge: bridgeAddress.base58,
    },
    max_fee_limit_sun_per_deploy_transaction: normalizeSun(
      options["fee-limit"],
      "--fee-limit",
      DEFAULT_DEPLOY_FEE_LIMIT_SUN,
    ),
    max_fee_limit_sun_per_trigger_transaction: normalizeSun(
      options["trigger-fee-limit"],
      "--trigger-fee-limit",
      DEFAULT_TRIGGER_FEE_LIMIT_SUN,
    ),
    steps,
    next_steps: broadcast
      ? [
          "Run evidence command with deployment_addresses.",
          "Run scripts/sccp_tron_live_evidence.py against the deployed verifier/source bridge.",
          "Activate TAIRA SCCP route evidence only after live readback and canary proofs match.",
        ]
      : [
          "Dry-run only: unsigned deploy transactions were created but no contracts were deployed.",
          `Re-run with --broadcast true --confirm-mainnet ${CONFIRMATION_TEXT} after funding the deployer.`,
        ],
  };
  const out = await writeJson(options.out ?? DEFAULT_DEPLOYMENT_OUT, plan);
  console.log(JSON.stringify({
    wrote: out,
    broadcast,
    deployer: deployer.address.base58,
    deployment_addresses: plan.deployment_addresses,
    next_steps: plan.next_steps,
  }, null, 2));
}

async function accountStatusCommand(options) {
  const deployer = await loadDeployerSecret(options.secret ?? DEFAULT_SECRET_OUT);
  const endpoint = options.endpoint ?? DEFAULT_TRON_ENDPOINT;
  const account = await tronPost(
    endpoint,
    "wallet/getaccount",
    { address: deployer.address.base58, visible: true },
    options,
  );
  const balanceSun = typeof account.balance === "number" ? account.balance : 0;
  console.log(JSON.stringify({
    endpoint,
    address_base58: deployer.address.base58,
    address_hex: deployer.address.hex,
    exists: Object.keys(account).length > 0,
    balance_sun: balanceSun,
    balance_trx: balanceSun / 1_000_000,
    raw: account,
  }, null, 2));
}

async function writeEvidence(options) {
  for (const key of ["token", "bridge", "source-bridge", "verifier"]) {
    if (!options[key]) throw new Error(`--${key} is required`);
  }
  const tokenAddress = normalizeTronBase58Address(options.token, "--token");
  const bridgeAddress = normalizeTronBase58Address(options.bridge, "--bridge");
  const sourceBridgeAddress = normalizeTronBase58Address(
    options["source-bridge"],
    "--source-bridge",
  );
  const verifierAddress = normalizeTronBase58Address(options.verifier, "--verifier");
  const uniqueAddresses = new Set([
    tokenAddress.base58,
    bridgeAddress.base58,
    sourceBridgeAddress.base58,
    verifierAddress.base58,
  ]);
  if (uniqueAddresses.size !== 4) {
    throw new Error("Token, bridge, source bridge, and verifier addresses must be distinct");
  }
  const evidence = {
    schema: EVIDENCE_SCHEMA,
    created_at: new Date().toISOString(),
    route_id: ROUTE_ID,
    route_id_hash: routeHash(ROUTE_ID),
    asset_key: ASSET_KEY,
    asset_key_hash: routeHash(ASSET_KEY),
    network: "tron-mainnet",
    network_id_hex: TRON_MAINNET_NETWORK_ID_HEX,
    taira_xor_token_address: tokenAddress.base58,
    taira_xor_token_address_hex: tokenAddress.hex,
    taira_xor_bridge_address: bridgeAddress.base58,
    taira_xor_bridge_address_hex: bridgeAddress.hex,
    sccp_tron_source_bridge_address: sourceBridgeAddress.base58,
    sccp_tron_source_bridge_address_hex: sourceBridgeAddress.hex,
    sccp_tron_destination_verifier_address: verifierAddress.base58,
    sccp_tron_destination_verifier_address_hex: verifierAddress.hex,
    required_post_deploy_checks: [
      "TairaXOR.bridge() equals taira_xor_bridge_address",
      "TairaXOR.bridgeLocked() is true",
      "SccpTronSourceBridge.owner() equals taira_xor_bridge_address",
      "TairaXorSccpBridge.destinationBindingHash() equals verifier destinationBindingHash()",
      "Run scripts/sccp_tron_source_bridge_evidence.py for source bridge config evidence",
      "Run scripts/sccp_tron_live_evidence.py for live verifier/source/canary evidence",
    ],
  };
  const out = await writeJson(options.out ?? DEFAULT_EVIDENCE_OUT, evidence);
  console.log(JSON.stringify({ wrote: out, evidence }, null, 2));
}

function expectThrows(fn, messagePattern) {
  try {
    fn();
  } catch (error) {
    if (messagePattern && !messagePattern.test(error.message)) {
      throw new Error(`Expected error ${messagePattern}, got: ${error.message}`);
    }
    return;
  }
  throw new Error("Expected function to throw");
}

function selfTest() {
  const privateKey = new Uint8Array(32).fill(1);
  const address = tronAddressFromPrivateKey(privateKey);
  if (!address.base58.startsWith("T") || !address.hex.startsWith("0x41")) {
    throw new Error("TRON address derivation self-test failed");
  }
  const normalized = normalizeTronBase58Address(address.base58, "self-test address");
  if (normalized.hex !== address.hex || normalized.solidity !== `0x${address.hex.slice(4)}`) {
    throw new Error("TRON address normalization self-test failed");
  }
  if (normalizeTronAddress(address.hex, "self-test address hex").base58 !== address.base58) {
    throw new Error("TRON hex address normalization self-test failed");
  }
  for (const invalid of [
    "",
    ` ${address.base58}`,
    `${address.base58.slice(0, -1)}${address.base58.endsWith("1") ? "2" : "1"}`,
    "1111111111111111111111111111111111",
    "0x410000000000000000000000000000000000000000",
  ]) {
    expectThrows(() => normalizeTronBase58Address(invalid, "invalid self-test address"));
  }
  if (routeHash(ROUTE_ID).length !== 66 || routeHash(ASSET_KEY).length !== 66) {
    throw new Error("Route hash self-test failed");
  }

  const mockRawData = bytesToHex(new Uint8Array([1, 2, 3, 4, 5]), false);
  const mockTx = {
    visible: true,
    txID: bytesToHex(sha256(hexToBytes(mockRawData, "mock raw data")), false),
    raw_data: {
      contract: [
        {
          parameter: {
            value: {
              owner_address: address.base58,
            },
          },
          type: "TriggerSmartContract",
        },
      ],
      timestamp: 1,
      expiration: 2,
    },
    raw_data_hex: mockRawData,
  };
  const signed = signTransactionPayload(mockTx, { privateKey, address });
  if (signed.signed.signature.length !== 1 || signed.metadata.txid !== mockTx.txID) {
    throw new Error("TRON signing self-test failed");
  }
  expectThrows(
    () => signTransactionPayload({ ...mockTx, txID: "00".repeat(32) }, { privateKey, address }),
    /txID/,
  );
  expectThrows(
    () =>
      signTransactionPayload(
        {
          ...mockTx,
          raw_data: {
            contract: [
              {
                parameter: {
                  value: {
                    owner_address: tronAddressFromPrivateKey(new Uint8Array(32).fill(2)).base58,
                  },
                },
              },
            ],
          },
        },
        { privateKey, address },
      ),
    /does not match deployer/,
  );
  expectThrows(
    () => signTransactionPayload({ ...mockTx, signature: [signed.metadata.signature] }, { privateKey, address }),
    /already signed/,
  );

  const verifierMaterial = {
    alpha1: [1, 2],
    beta2: [[3, 4], [5, 6]],
    gamma2: [7, 8, 9, 10],
    delta2: [11, 12, 13, 14],
    ic: [15, 16, 17, 18],
    verifierKeyHash: routeHash("verifier-key"),
  };
  const verifierArgs = normalizeVerifierConstructorArgs(verifierMaterial);
  if (verifierArgs.length !== 10 || verifierArgs[5] !== verifierMaterial.verifierKeyHash) {
    throw new Error("Verifier material normalization self-test failed");
  }
  expectThrows(
    () => normalizeVerifierConstructorArgs({ ...verifierMaterial, proofFamily: "debug" }),
    /proofFamily/,
  );
  expectThrows(
    () => normalizeVerifierConstructorArgs({ ...verifierMaterial, networkId: routeHash("wrong-network") }),
    /networkId/,
  );

  const duplicateEvidenceAddress = address.base58;
  expectThrows(() => {
    const uniqueAddresses = new Set([
      duplicateEvidenceAddress,
      duplicateEvidenceAddress,
      tronAddressFromPrivateKey(new Uint8Array(32).fill(3)).base58,
      tronAddressFromPrivateKey(new Uint8Array(32).fill(4)).base58,
    ]);
    if (uniqueAddresses.size !== 4) {
      throw new Error("Token, bridge, source bridge, and verifier addresses must be distinct");
    }
  }, /distinct/);

  console.log("sccp_tron_taira_xor_deploy: self-test ok");
}

async function main() {
  if (process.argv.includes("--help") || process.argv.includes("-h")) {
    console.log(usage());
    return;
  }
  const { command, options } = parseArgs(process.argv.slice(2));
  if (command === "generate-deployer") return generateDeployer(options);
  if (command === "account-status") return accountStatusCommand(options);
  if (command === "compile") return compileCommand(options);
  if (command === "compile-taira-contract") return compileTairaContractCommand(options);
  if (command === "deploy") return deployCommand(options);
  if (command === "sign-transaction") return signTransactionCommand(options);
  if (command === "broadcast") return broadcastCommand(options);
  if (command === "evidence") return writeEvidence(options);
  if (command === "self-test") return selfTest();
  throw new Error(usage());
}

if (process.argv[1] && resolve(process.argv[1]) === SCRIPT_PATH) {
  main().catch((error) => {
    console.error(error.message || error);
    process.exitCode = 1;
  });
}

export {
  ASSET_KEY,
  ROUTE_ID,
  TRON_MAINNET_NETWORK_ID_HEX,
  bytesToHex,
  compileTairaBurnRecordContract,
  hexToBytes,
  normalizeTronAddress,
  normalizeTronBase58Address,
  normalizeVerifierConstructorArgs,
  routeHash,
  signTransactionPayload,
  tronAddressFromPrivateKey,
};
