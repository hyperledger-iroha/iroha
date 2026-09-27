"use strict";

// EDR harness for `SccpTairaXor.sol`: authenticated artifact loading through
// `scripts/contract_artifact_corridor.py`, deployment on the locked native EDR
// runtime, explicit block-time control, strict custom-error decoding and gas
// capture. Requires Python 3 and the locked `scripts/contract_tooling/evm-runtime`
// dependencies; `SCCP_CONTRACT_ARTIFACT_MANIFEST` may name an already built
// manifest (developer convenience only).

const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const model = require("./sccp_v1_model.js");

const { ethers, REPO, RUNTIME_DIR } = model;
const { createNativeEdrProvider } = require(path.join(RUNTIME_DIR, "edr-provider.js"));

const CONTRACT = "contracts/evm/sccp/SccpTairaXor.sol:SccpTairaXor";
const CORRIDOR = path.join(REPO, "scripts", "contract_artifact_corridor.py");
const DEFAULT_MANIFEST = path.join(REPO, "target", "sccp-contract-artifacts", "sccp-contract-artifacts-v1.json");
const ARTIFACT_LOCK = path.join(REPO, "scripts", "contract_tooling", "artifact-lock.json");
const BLOCK_GAS_LIMIT = 60_000_000;
const TX_GAS_LIMIT = 16_000_000n;

function runCorridor(args) {
  const python = process.env.SCCP_CORRIDOR_PYTHON_BIN || "python3";
  return spawnSync(python, [CORRIDOR, ...args], { cwd: REPO, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
}

function requireCorridor(args) {
  const result = runCorridor(args);
  assert.equal(result.status, 0, `contract artifact corridor ${args[0]} failed: ${result.stderr || result.error?.message}`);
}

let cachedArtifacts = null;

/**
 * Loads the SccpTairaXor artifact from a manifest that the corridor has just
 * re-verified against the compiler lock, the artifact lock and this checkout.
 * The default manifest is (re)built when it is missing or stale.
 */
function loadArtifacts() {
  if (cachedArtifacts) return cachedArtifacts;
  const explicit = process.env.SCCP_CONTRACT_ARTIFACT_MANIFEST;
  const manifestPath = explicit ? path.resolve(explicit) : DEFAULT_MANIFEST;
  const verify = ["verify", "--manifest", manifestPath];
  if (explicit) {
    requireCorridor(verify);
  } else if (!fs.existsSync(manifestPath) || runCorridor(verify).status !== 0) {
    requireCorridor(["build"]);
    requireCorridor(verify);
  }
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  const pick = (target) => {
    const found = manifest.targets[target].contracts.filter((entry) => entry.fully_qualified_name === CONTRACT);
    assert.equal(found.length, 1, `${target} manifest must contain exactly one ${CONTRACT}`);
    return found[0];
  };
  const evm = pick("evm");
  const tron = pick("tron");
  const lock = JSON.parse(fs.readFileSync(ARTIFACT_LOCK, "utf8"));
  cachedArtifacts = Object.freeze({
    manifestPath,
    evm,
    tron,
    lock,
    abi: evm.abi,
    iface: new ethers.Interface(evm.abi),
    creation: evm.creation_bytecode.hex,
    runtimeTemplate: evm.runtime_bytecode.hex,
    immutableReferences: evm.runtime_immutable_references,
  });
  return cachedArtifacts;
}

/** Revert data of a failed EDR request, or null when the failure is not a plain revert. */
function revertData(error) {
  for (const candidate of [error, error?.error, error?.cause]) {
    const data = candidate?.data;
    if (data && typeof data === "object" && typeof data.data === "string" && data.reason
        && Object.keys(data.reason).length === 1 && data.reason.Revert === data.data) {
      return data.data;
    }
  }
  return null;
}

/** Awaits `promise` and requires a revert with exactly the named custom error (and arguments). */
async function expectRevert(promise, name, args) {
  let failure = null;
  try {
    await promise;
  } catch (error) {
    failure = error;
  }
  assert(failure, `expected revert ${name}, but the call succeeded`);
  const data = revertData(failure);
  assert(data !== null, `expected revert ${name}, got ${failure.message}`);
  if (name === null) {
    assert.equal(data, "0x", "expected an empty revert");
    return;
  }
  const parsed = loadArtifacts().iface.parseError(data);
  assert(parsed, `expected revert ${name}, got undecodable data ${data}`);
  assert.equal(parsed.name, name, `expected revert ${name}, got ${parsed.name}`);
  if (args !== undefined) {
    assert.deepEqual([...parsed.args].map((value) => (typeof value === "bigint" ? value : String(value))), args);
  }
}

/** One isolated EDR chain with explicit, strictly increasing block timestamps. */
class Chain {
  static async open(profile) {
    const provider = createNativeEdrProvider({ chainId: Number(profile.chainId), blockGasLimit: BLOCK_GAS_LIMIT });
    const chain = new Chain(profile, provider);
    chain.accounts = await provider.request({ method: "eth_accounts" });
    const latest = await provider.request({ method: "eth_getBlockByNumber", params: ["latest", false] });
    chain.time = BigInt(latest.timestamp);
    return chain;
  }

  constructor(profile, provider) {
    this.profile = profile;
    this.provider = provider;
    this.accounts = [];
    this.time = 0n;
  }

  rpc(method, params = []) {
    return this.provider.request({ method, params });
  }

  /** Timestamp (seconds) of the latest block, in milliseconds. */
  nowMs() {
    return this.time * 1000n;
  }

  /** Sends one transaction mined at `at` seconds (default: one second after the latest block). */
  async send({ from = 0, to, data, at, value }) {
    const timestamp = at === undefined ? this.time + 1n : BigInt(at);
    assert(timestamp > this.time, `block timestamps must increase (${timestamp} <= ${this.time})`);
    await this.rpc("evm_setNextBlockTimestamp", [ethers.toQuantity(timestamp)]);
    this.time = timestamp;
    const tx = { from: typeof from === "number" ? this.accounts[from] : from, data, gas: ethers.toQuantity(TX_GAS_LIMIT) };
    if (to !== undefined) tx.to = to;
    if (value !== undefined) tx.value = ethers.toQuantity(value);
    const hash = await this.rpc("eth_sendTransaction", [tx]);
    const receipt = await this.rpc("eth_getTransactionReceipt", [hash]);
    assert.equal(receipt.status, "0x1", "mined transaction must succeed");
    return receipt;
  }

  /** Read-only call against the latest block (its timestamp and state). */
  call(to, data, from) {
    const request = { to, data };
    if (from !== undefined) request.from = typeof from === "number" ? this.accounts[from] : from;
    return this.rpc("eth_call", [request, "latest"]);
  }

  close() {
    return this.provider.disconnect();
  }
}

/** A deployed SccpTairaXor with ABI helpers. */
class Destination {
  constructor(chain, address) {
    this.chain = chain;
    this.address = ethers.getAddress(address);
    this.iface = loadArtifacts().iface;
  }

  async view(name, args = []) {
    const output = await this.chain.call(this.address, this.iface.encodeFunctionData(name, args));
    const decoded = this.iface.decodeFunctionResult(name, output);
    return decoded.length === 1 ? decoded[0] : decoded;
  }

  /** Simulates `name` against the latest block and returns its decoded result. */
  async simulate(name, args, from = 0) {
    const output = await this.chain.call(this.address, this.iface.encodeFunctionData(name, args), from);
    const decoded = this.iface.decodeFunctionResult(name, output);
    return decoded.length === 1 ? decoded[0] : decoded;
  }

  tx(name, args, options = {}) {
    return this.chain.send({ ...options, to: this.address, data: this.iface.encodeFunctionData(name, args) });
  }

  raw(data, options = {}) {
    return this.chain.send({ ...options, to: this.address, data });
  }

  /** Logs of this contract in `receipt`, parsed; `topic0` is checked against the independent model. */
  events(receipt, name) {
    return receipt.logs
      .filter((log) => ethers.getAddress(log.address) === this.address)
      .map((log) => ({ log, parsed: this.iface.parseLog({ topics: log.topics, data: log.data }) }))
      .filter((entry) => entry.parsed && (name === undefined || entry.parsed.name === name));
  }
}

/** Deploys SccpTairaXor with the given constructor arguments. */
async function deploy(chain, { networkId, tag = chain.profile.tag, revision, cap, roster, at, from = 0 }) {
  const { iface, creation } = loadArtifacts();
  const data = ethers.concat([creation, iface.encodeDeploy([networkId, tag, revision, cap, model.rosterArg(roster)])]);
  const receipt = await chain.send({ from, data, at });
  return { destination: new Destination(chain, receipt.contractAddress), receipt };
}

/**
 * Deploys a minimal forwarder whose runtime relays its calldata to `target`
 * with CALL and bubbles the result, so `msg.sender != tx.origin` at the target.
 */
async function deployForwarder(chain, target) {
  const runtime = ethers.concat([
    "0x365f5f375f5f365f5f73",
    target,
    "0x5af13d5f5f3e602a573d5ffd5b3d5ff3",
  ]);
  assert.equal(ethers.getBytes(runtime).length, 46, "forwarder runtime layout");
  const init = ethers.concat(["0x602e8060095f395ff3", runtime]);
  const receipt = await chain.send({ data: init });
  return ethers.getAddress(receipt.contractAddress);
}

/** Runtime bytecode the corridor template yields once the named immutables are filled. */
function expectedRuntime(values) {
  const { runtimeTemplate, immutableReferences } = loadArtifacts();
  const bytes = ethers.getBytes(runtimeTemplate);
  for (const reference of immutableReferences) {
    assert(Object.hasOwn(values, reference.name), `missing immutable ${reference.name}`);
    bytes.set(ethers.getBytes(values[reference.name]), reference.start);
  }
  return ethers.hexlify(bytes);
}

/**
 * Recompiles the checkout source with both pinned native compilers through
 * `scripts/contract_native_solc.js` and returns `{ target: SccpTairaXor output }`.
 */
function recompileWithNativeCompilers() {
  const { compileNativeSolidity } = require(path.join(REPO, "scripts", "contract_native_solc.js"));
  const lock = JSON.parse(fs.readFileSync(path.join(REPO, "scripts", "contract_tooling", "compiler-lock.json"), "utf8"));
  const [sourcePath, contractName] = CONTRACT.split(":");
  const input = JSON.stringify({
    language: "Solidity",
    sources: { [sourcePath]: { content: fs.readFileSync(path.join(REPO, sourcePath), "utf8") } },
    settings: lock.settings,
  });
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "sccp-native-solc-"));
  try {
    const outputs = {};
    for (const target of ["evm", "tron"]) {
      const compilerPath = path.join(directory, `${target}-solc`);
      requireCorridor(["materialize", "--target", target, "--output", compilerPath]);
      const output = compileNativeSolidity(input, {
        target,
        compilerPath,
        pythonBin: process.env.SCCP_CORRIDOR_PYTHON_BIN || "python3",
      });
      outputs[target] = output.contracts[sourcePath][contractName];
    }
    return outputs;
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
}

/** Measured gas per operation and profile, printed after the suite. */
const GAS = new Map();

function recordGas(profile, label, receipt) {
  const gas = BigInt(receipt.gasUsed);
  const key = `${label}`;
  const row = GAS.get(key) ?? {};
  row[profile.name] = gas;
  GAS.set(key, row);
  return gas;
}

function gasReport() {
  const rows = [...GAS.entries()].sort(([a], [b]) => a.localeCompare(b));
  const lines = ["| Operation | ETH (chain 1) | BSC (chain 56) | TRON profile on EDR |", "|---|---|---|---|"];
  for (const [label, row] of rows) {
    lines.push(`| ${label} | ${row.ethereum ?? "-"} | ${row.bsc ?? "-"} | ${row.tron ?? "-"} |`);
  }
  return lines.join("\n");
}

function writeGasReport() {
  const target = process.env.SCCP_GAS_REPORT;
  if (!target) return;
  const json = Object.fromEntries(
    [...GAS.entries()].map(([label, row]) => [label, Object.fromEntries(Object.entries(row).map(([k, v]) => [k, Number(v)]))]),
  );
  fs.writeFileSync(target, `${JSON.stringify(json, null, 2)}\n`);
}

module.exports = {
  CONTRACT,
  Chain,
  Destination,
  GAS,
  deploy,
  deployForwarder,
  expectRevert,
  expectedRuntime,
  gasReport,
  loadArtifacts,
  recompileWithNativeCompilers,
  recordGas,
  revertData,
  writeGasReport,
};
