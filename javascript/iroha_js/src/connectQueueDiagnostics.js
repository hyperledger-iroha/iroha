import fs from "node:fs/promises";
import path from "node:path";
import os from "node:os";

const SCHEMA_VERSION = 1;
const CONNECT_QUEUE_ROOT_ENV = "IROHA_CONNECT_QUEUE_ROOT";
const DEFAULT_CONNECT_QUEUE_ROOT = path.join(os.homedir(), ".iroha", "connect");

function bufferFromSessionId(sessionId) {
  if (sessionId instanceof Uint8Array) {
    return Buffer.from(sessionId);
  }
  if (typeof sessionId === "string") {
    const trimmed = sessionId.trim();
    if (!trimmed) {
      throw new TypeError("Connect session id must be non-empty");
    }
    const decoded = tryDecodeBase64Url(trimmed);
    return decoded ?? Buffer.from(trimmed, "utf8");
  }
  throw new TypeError("Connect session id must be a string or Uint8Array");
}

function tryDecodeBase64Url(value) {
  const normalized = value.replace(/-/g, "+").replace(/_/g, "/");
  let padded = normalized;
  const paddingIndex = normalized.indexOf("=");
  if (paddingIndex !== -1) {
    const head = normalized.slice(0, paddingIndex);
    const padding = normalized.slice(paddingIndex);
    if (!/^[0-9A-Za-z+/]*$/.test(head) || !/^={1,2}$/.test(padding)) {
      return null;
    }
    if (normalized.length % 4 !== 0) {
      return null;
    }
  } else {
    if (!/^[0-9A-Za-z+/]+$/.test(normalized) || normalized.length % 4 === 1) {
      return null;
    }
    const padLength = (4 - (normalized.length % 4)) % 4;
    padded = normalized + "=".repeat(padLength);
  }
  const decoded = Buffer.from(padded, "base64");
  if (decoded.toString("base64") !== padded) {
    return null;
  }
  return decoded;
}

function toBase64Url(buffer) {
  return Buffer.from(buffer)
    .toString("base64")
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/u, "");
}

function allowEnvOverride(options) {
  return Boolean(options?.allowEnvOverride);
}

function connectQueueRootFromConfig(connectConfig) {
  if (!connectConfig) {
    return null;
  }
  if (typeof connectConfig === "string") {
    const trimmed = connectConfig.trim();
    return trimmed.length > 0 ? path.resolve(trimmed) : null;
  }
  if (typeof connectConfig !== "object") {
    return null;
  }
  const candidate =
    connectConfig.connect?.queue?.root ??
    connectConfig.connect?.queue_root ??
    connectConfig.connect_queue_root ??
    connectConfig.connectQueueRoot;
  if (typeof candidate === "string" && candidate.trim().length > 0) {
    return path.resolve(candidate);
  }
  return null;
}

function envConnectQueueRoot(allowEnv) {
  if (!allowEnv) {
    return null;
  }
  const override = process.env[CONNECT_QUEUE_ROOT_ENV];
  if (!override) {
    return null;
  }
  const trimmed = override.trim();
  return trimmed.length > 0 ? path.resolve(trimmed) : null;
}

function resolveConnectQueueRoot(options = {}) {
  if (options.rootDir) {
    return path.resolve(options.rootDir);
  }
  const configRoot = connectQueueRootFromConfig(options.connectConfig);
  if (configRoot) {
    return configRoot;
  }
  const envRoot = envConnectQueueRoot(allowEnvOverride(options));
  if (envRoot) {
    return envRoot;
  }
  return DEFAULT_CONNECT_QUEUE_ROOT;
}

export function defaultConnectQueueRoot(options = {}) {
  return resolveConnectQueueRoot(options);
}

export function deriveConnectSessionDirectory({ sid, rootDir, connectConfig, allowEnvOverride: allowEnv }) {
  const normalized = toBase64Url(bufferFromSessionId(sid));
  const resolvedRoot = resolveConnectQueueRoot({ rootDir, connectConfig, allowEnvOverride: allowEnv });
  return path.join(resolvedRoot, normalized);
}

function statePathFor(options) {
  if (options.snapshotPath) {
    return options.snapshotPath;
  }
  const rootDir = resolveConnectQueueRoot(options);
  const sessionDir = deriveConnectSessionDirectory({
    sid: options.sid,
    rootDir,
    connectConfig: options.connectConfig,
    allowEnvOverride: options.allowEnvOverride,
  });
  return path.join(sessionDir, "state.json");
}

function createDefaultSnapshot(sid, options = {}) {
  const sessionBytes = bufferFromSessionId(sid);
  return {
    schema_version: SCHEMA_VERSION,
    session_id_base64: toBase64Url(sessionBytes),
    state: "disabled",
    reason: null,
    warning_watermark: options.warningWatermark ?? 0.6,
    drop_watermark: options.dropWatermark ?? 0.85,
    last_updated_ms: Date.now(),
    app_to_wallet: {
      depth: 0,
      bytes: 0,
      oldest_sequence: null,
      newest_sequence: null,
      oldest_timestamp_ms: null,
      newest_timestamp_ms: null,
    },
    wallet_to_app: {
      depth: 0,
      bytes: 0,
      oldest_sequence: null,
      newest_sequence: null,
      oldest_timestamp_ms: null,
      newest_timestamp_ms: null,
    },
  };
}

export async function readConnectQueueSnapshot(options) {
  const resolvedPath = statePathFor(options);
  try {
    const content = await fs.readFile(resolvedPath, "utf8");
    return { snapshot: JSON.parse(content), statePath: resolvedPath };
  } catch (error) {
    if (error && error.code === "ENOENT") {
      return {
        snapshot: createDefaultSnapshot(options.sid ?? "unknown", options),
        statePath: resolvedPath,
      };
    }
    throw error;
  }
}

async function ensureSessionDirectory(sid, rootDir) {
  const sessionDir = deriveConnectSessionDirectory({ sid, rootDir });
  await fs.mkdir(sessionDir, { recursive: true });
  return sessionDir;
}

export async function writeConnectQueueSnapshot(snapshot, options = {}) {
  const rootDir = resolveConnectQueueRoot(options);
  const sessionDir = await ensureSessionDirectory(snapshot.session_id_base64 ?? options.sid, rootDir);
  const statePath = path.join(sessionDir, "state.json");
  const payload = {
    ...snapshot,
    schema_version: snapshot.schema_version ?? SCHEMA_VERSION,
    last_updated_ms: snapshot.last_updated_ms ?? Date.now(),
  };
  await fs.writeFile(statePath, JSON.stringify(payload, null, 2), "utf8");
  return { snapshot: payload, statePath };
}

export async function updateConnectQueueSnapshot(sid, updater, options = {}) {
  const rootDir = resolveConnectQueueRoot(options);
  const sessionDir = await ensureSessionDirectory(sid, rootDir);
  const statePath = path.join(sessionDir, "state.json");
  let current = createDefaultSnapshot(sid, options);
  try {
    const content = await fs.readFile(statePath, "utf8");
    current = JSON.parse(content);
  } catch (error) {
    if (!error || error.code !== "ENOENT") {
      throw error;
    }
  }
  const updated = typeof updater === "function" ? updater({ ...current }) : { ...current, ...updater };
  updated.schema_version = SCHEMA_VERSION;
  updated.session_id_base64 = updated.session_id_base64 ?? toBase64Url(bufferFromSessionId(sid));
  updated.last_updated_ms = Date.now();
  await fs.writeFile(statePath, JSON.stringify(updated, null, 2), "utf8");
  return updated;
}

export async function appendConnectQueueMetric(sid, sample, options = {}) {
  const rootDir = resolveConnectQueueRoot(options);
  const sessionDir = await ensureSessionDirectory(sid, rootDir);
  const metricsPath = path.join(sessionDir, "metrics.ndjson");
  const payload = {
    timestamp_ms: sample.timestamp_ms ?? Date.now(),
    state: sample.state ?? "disabled",
    app_to_wallet_depth: sample.app_to_wallet_depth ?? 0,
    wallet_to_app_depth: sample.wallet_to_app_depth ?? 0,
    reason: sample.reason ?? null,
  };
  await fs.appendFile(metricsPath, `${JSON.stringify(payload)}\n`, "utf8");
  return metricsPath;
}

async function copyIfExists(sourceDir, fileName, targetDir) {
  const src = path.join(sourceDir, fileName);
  try {
    await fs.access(src);
  } catch (error) {
    if (error && error.code === "ENOENT") {
      return null;
    }
    throw error;
  }
  const dest = path.join(targetDir, fileName);
  await fs.copyFile(src, dest);
  return fileName;
}

export async function exportConnectQueueEvidence(sid, targetDir, options = {}) {
  const rootDir = resolveConnectQueueRoot(options);
  const sessionDir = deriveConnectSessionDirectory({ sid, rootDir });
  await fs.mkdir(targetDir, { recursive: true });
  const { snapshot } = await readConnectQueueSnapshot({ sid, rootDir });
  const files = {};
  const appQueue = await copyIfExists(sessionDir, "app_to_wallet.queue", targetDir);
  if (appQueue) files.app_queue_filename = appQueue;
  const walletQueue = await copyIfExists(sessionDir, "wallet_to_app.queue", targetDir);
  if (walletQueue) files.wallet_queue_filename = walletQueue;
  const metrics = await copyIfExists(sessionDir, "metrics.ndjson", targetDir);
  if (metrics) files.metrics_filename = metrics;
  const manifest = {
    schema_version: SCHEMA_VERSION,
    session_id_base64: snapshot.session_id_base64,
    created_at_ms: Date.now(),
    snapshot,
    files,
  };
  await fs.writeFile(
    path.join(targetDir, "manifest.json"),
    JSON.stringify(manifest, null, 2),
    "utf8",
  );
  return { manifest, targetDir };
}
