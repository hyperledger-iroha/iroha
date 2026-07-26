// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import { dirname } from "node:path";
import { mkdir, appendFile } from "node:fs/promises";

function ensureToriiClient(client) {
  if (!client || typeof client.getSumeragiTelemetryTyped !== "function") {
    throw new TypeError("client must expose getSumeragiTelemetryTyped()");
  }
}

function isPlainObject(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return false;
  }
  const proto = Object.getPrototypeOf(value);
  return proto === Object.prototype || proto === null;
}

function isAbortSignalLike(value) {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof value.aborted === "boolean" &&
    typeof value.addEventListener === "function" &&
    typeof value.removeEventListener === "function"
  );
}

function normalizeCaptureOptions(options, context, extraAllowedKeys = []) {
  if (options === undefined) {
    return { signal: undefined, timestamp: undefined };
  }
  if (!isPlainObject(options)) {
    throw new TypeError(`${context} options must be a plain object`);
  }
  const unsupported = Object.keys(options).filter(
    (key) => key !== "signal" && key !== "timestamp" && !extraAllowedKeys.includes(key),
  );
  if (unsupported.length > 0) {
    throw new TypeError(
      `${context} options contains unsupported fields: ${unsupported.join(", ")}`,
    );
  }
  const { signal, timestamp } = options;
  if (signal !== undefined && signal !== null && !isAbortSignalLike(signal)) {
    throw new TypeError(`${context} options.signal must be an AbortSignal`);
  }
  if (timestamp !== undefined && timestamp !== null) {
    if (typeof timestamp !== "number" || !Number.isFinite(timestamp)) {
      throw new TypeError(`${context} options.timestamp must be a finite number`);
    }
  }
  return {
    signal: signal ?? undefined,
    timestamp:
      typeof timestamp === "number" && Number.isFinite(timestamp) ? timestamp : undefined,
  };
}

function normalizeFs(overrides = {}, context = "appendSumeragiTelemetrySnapshot options.fs") {
  if (overrides === undefined) {
    return {
      mkdir,
      appendFile,
    };
  }
  if (!isPlainObject(overrides)) {
    throw new TypeError(`${context} must be a plain object when provided`);
  }
  const { mkdir: mkdirOverride, appendFile: appendFileOverride } = overrides;
  if (mkdirOverride !== undefined && typeof mkdirOverride !== "function") {
    throw new TypeError(`${context}.mkdir must be a function when provided`);
  }
  if (appendFileOverride !== undefined && typeof appendFileOverride !== "function") {
    throw new TypeError(`${context}.appendFile must be a function when provided`);
  }
  return {
    mkdir: mkdirOverride ?? mkdir,
    appendFile: appendFileOverride ?? appendFile,
  };
}

function normalizeAppendOptions(options) {
  const { signal, timestamp } = normalizeCaptureOptions(
    options,
    "appendSumeragiTelemetrySnapshot",
    ["fs"],
  );
  const fsOverrides = options && Object.prototype.hasOwnProperty.call(options, "fs")
    ? options.fs
    : undefined;
  const fs = normalizeFs(fsOverrides, "appendSumeragiTelemetrySnapshot options.fs");
  return { signal, timestamp, fs };
}

/**
 * Capture a typed `/v1/sumeragi/telemetry` snapshot and annotate it with a timestamp.
 * @param {import("./toriiClient.js").ToriiClient} client
 * @param {{signal?: AbortSignal, timestamp?: number}} [options]
 * @returns {Promise<{capturedAtUnixMs: number, capturedAtIso: string, telemetry: import("../index.d.ts").SumeragiTelemetrySnapshot}>}
 */
export async function captureSumeragiTelemetrySnapshot(client, options = {}) {
  ensureToriiClient(client);
  const { signal, timestamp } = normalizeCaptureOptions(
    options,
    "captureSumeragiTelemetrySnapshot",
  );
  const capturedAtUnixMs = timestamp ?? Date.now();
  const telemetry = await client.getSumeragiTelemetryTyped({ signal });
  return {
    capturedAtUnixMs,
    capturedAtIso: new Date(capturedAtUnixMs).toISOString(),
    telemetry,
  };
}

/**
 * Capture a telemetry snapshot and append it to an NDJSON file for replay.
 * @param {import("./toriiClient.js").ToriiClient} client
 * @param {string} outputPath
 * @param {{signal?: AbortSignal, timestamp?: number, fs?: Partial<typeof import("node:fs/promises")>}} [options]
 * @returns {Promise<{capturedAtUnixMs: number, capturedAtIso: string, telemetry: import("../index.d.ts").SumeragiTelemetrySnapshot}>}
 */
export async function appendSumeragiTelemetrySnapshot(client, outputPath, options = {}) {
  if (typeof outputPath !== "string" || outputPath.trim() === "") {
    throw new TypeError("outputPath must be a non-empty string");
  }
  const { signal, timestamp, fs: fsImpl } = normalizeAppendOptions(options);
  const snapshot = await captureSumeragiTelemetrySnapshot(client, { signal, timestamp });
  const dir = dirname(outputPath);
  if (dir && dir !== ".") {
    await fsImpl.mkdir(dir, { recursive: true });
  }
  await fsImpl.appendFile(outputPath, `${JSON.stringify(snapshot)}\n`);
  return snapshot;
}
