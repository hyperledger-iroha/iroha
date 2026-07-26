"use strict";

const fs = require("fs");
const net = require("net");
const os = require("os");
const path = require("path");
const { spawn } = require("child_process");

const MAX_START_ATTEMPTS = 120;
const REQUEST_TIMEOUT_MS = 30_000;

function reserveLoopbackPort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.unref();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      const port = typeof address === "object" && address ? address.port : 0;
      server.close((error) => {
        if (error) reject(error);
        else if (!Number.isInteger(port) || port <= 0) {
          reject(new Error("failed to reserve a loopback port"));
        } else resolve(port);
      });
    });
  });
}

function hardhatCliPath() {
  const entry = require.resolve("hardhat");
  const candidate = path.join(path.dirname(entry), "cli.js");
  const info = fs.lstatSync(candidate);
  if (!info.isFile() || info.isSymbolicLink()) {
    throw new Error("locked Hardhat package does not expose its direct CLI file");
  }
  return candidate;
}

class HardhatEip1193Provider {
  constructor({ chainId, blockGasLimit }) {
    if (!Number.isSafeInteger(chainId) || chainId <= 0) {
      throw new Error("Hardhat provider requires a positive safe chainId");
    }
    if (!Number.isSafeInteger(blockGasLimit) || blockGasLimit <= 0) {
      throw new Error("Hardhat provider requires a positive safe blockGasLimit");
    }
    this.chainId = chainId;
    this.blockGasLimit = blockGasLimit;
    this.child = null;
    this.workDir = null;
    this.endpoint = null;
    this.requestId = 0;
    this.exitHandler = () => {
      if (this.child && this.child.exitCode === null) this.child.kill("SIGTERM");
      if (this.workDir) fs.rmSync(this.workDir, { recursive: true, force: true });
    };
    process.once("exit", this.exitHandler);
    this.startPromise = this.start();
  }

  async start() {
    const port = await reserveLoopbackPort();
    this.workDir = fs.mkdtempSync(path.join(os.tmpdir(), "iroha-sccp-hardhat-"));
    fs.writeFileSync(
      path.join(this.workDir, "package.json"),
      '{"name":"iroha-sccp-hardhat-runtime","private":true,"type":"module"}\n',
      { encoding: "utf8", flag: "wx", mode: 0o600 },
    );
    const configPath = path.join(this.workDir, "hardhat.config.mjs");
    const config = {
      defaultNetwork: "hardhat",
      networks: {
        hardhat: {
          type: "edr-simulated",
          chainType: "l1",
          chainId: this.chainId,
          blockGasLimit: this.blockGasLimit,
          allowUnlimitedContractSize: false,
          // Fail the EIP-1193 send immediately on a revert. The smoke harness
          // validates Hardhat's bounded code/data/reason error shape directly,
          // so every negative case observes rejection before a tx hash exists.
          throwOnTransactionFailures: true,
        },
      },
    };
    fs.writeFileSync(configPath, `export default ${JSON.stringify(config)};\n`, {
      encoding: "utf8",
      flag: "wx",
      mode: 0o600,
    });
    const logDescriptor = fs.openSync(
      path.join(this.workDir, "hardhat.log"),
      "wx",
      0o600,
    );
    this.child = spawn(
      process.execPath,
      [
        hardhatCliPath(),
        "--config",
        configPath,
        "--network",
        "hardhat",
        "node",
        "--hostname",
        "127.0.0.1",
        "--port",
        String(port),
      ],
      {
        cwd: this.workDir,
        env: { ...process.env, HARDHAT_DISABLE_TELEMETRY_PROMPT: "true" },
        stdio: ["ignore", logDescriptor, logDescriptor],
      },
    );
    fs.closeSync(logDescriptor);
    this.endpoint = `http://127.0.0.1:${port}`;
    for (let attempt = 0; attempt < MAX_START_ATTEMPTS; attempt += 1) {
      if (this.child.exitCode !== null) {
        throw new Error("locked Hardhat node exited before becoming ready");
      }
      try {
        const reported = await this.rawRequest("eth_chainId", []);
        if (BigInt(reported) !== BigInt(this.chainId)) {
          throw new Error(`locked Hardhat node reported the wrong chain id: ${reported}`);
        }
        return;
      } catch (error) {
        if (attempt + 1 === MAX_START_ATTEMPTS) throw error;
        await new Promise((resolve) => setTimeout(resolve, 100));
      }
    }
  }

  async rawRequest(method, params) {
    const response = await fetch(this.endpoint, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: (this.requestId += 1),
        method,
        params,
      }),
      signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
    });
    if (!response.ok) {
      throw new Error(`Hardhat JSON-RPC returned HTTP ${response.status}`);
    }
    const payload = await response.json();
    if (payload.error) {
      const error = new Error(payload.error.message || "Hardhat JSON-RPC error");
      error.code = payload.error.code;
      error.data = payload.error.data;
      throw error;
    }
    return payload.result;
  }

  async request({ method, params = [] }) {
    if (typeof method !== "string" || !Array.isArray(params)) {
      throw new TypeError("EIP-1193 request must contain a method and parameter array");
    }
    await this.startPromise;
    return this.rawRequest(method, params);
  }

  async disconnect() {
    try {
      await this.startPromise;
    } catch (_error) {
      // The original request still owns and reports any startup failure.
    }
    if (this.child && this.child.exitCode === null) {
      this.child.kill("SIGTERM");
      await new Promise((resolve) => {
        const timeout = setTimeout(resolve, 5_000);
        this.child.once("exit", () => {
          clearTimeout(timeout);
          resolve();
        });
      });
    }
    if (this.workDir) fs.rmSync(this.workDir, { recursive: true, force: true });
    process.removeListener("exit", this.exitHandler);
  }
}

function createHardhatProvider(options) {
  return new HardhatEip1193Provider(options);
}

module.exports = { createHardhatProvider };
