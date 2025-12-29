#!/usr/bin/env node
// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import path from "node:path";
import process from "node:process";
import { setTimeout as delay } from "node:timers/promises";
import { parseArgs } from "node:util";
import { fileURLToPath } from "node:url";

import { ToriiClient } from "../src/toriiClient.js";
import { appendSumeragiTelemetrySnapshot } from "../src/telemetryReplay.js";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const JS_DIR = path.resolve(SCRIPT_DIR, "..");
const REPO_ROOT = path.resolve(JS_DIR, "..");
const DEFAULT_OUTPUT = path.join(REPO_ROOT, "artifacts", "sumeragi_telemetry", "latest.ndjson");

const DEFAULT_TORII_URL =
  process.env.JS_TORII_URL ?? process.env.IROHA_TORII_URL ?? "http://127.0.0.1:8080";
const DEFAULT_API_TOKEN = process.env.JS_TORII_API_TOKEN ?? process.env.IROHA_TORII_API_TOKEN;
const DEFAULT_AUTH_TOKEN = process.env.JS_TORII_AUTH_TOKEN ?? process.env.IROHA_TORII_AUTH_TOKEN;

function parseInteger(value, fallback) {
  if (value === undefined) {
    return fallback;
  }
  const num = Number.parseInt(value, 10);
  if (Number.isNaN(num)) {
    throw new Error(`invalid integer: ${value}`);
  }
  return num;
}

async function main() {
  const { values } = parseArgs({
    options: {
      "torii-url": { type: "string" },
      output: { type: "string" },
      samples: { type: "string" },
      "interval-ms": { type: "string" },
      "api-token": { type: "string" },
      "auth-token": { type: "string" },
      "timeout-ms": { type: "string" },
    },
  });

  const baseUrl = values["torii-url"] ?? DEFAULT_TORII_URL;
  const samples = parseInteger(values.samples, 1);
  if (samples <= 0) {
    throw new Error("samples must be >= 1");
  }
  const intervalMs = parseInteger(values["interval-ms"], 1_000);
  if (intervalMs < 0) {
    throw new Error("interval-ms must be >= 0");
  }
  const timeoutMs = values["timeout-ms"] ? parseInteger(values["timeout-ms"], 0) : undefined;
  const apiToken = values["api-token"] ?? DEFAULT_API_TOKEN;
  const authToken = values["auth-token"] ?? DEFAULT_AUTH_TOKEN;
  const outputPathInput = values.output ?? DEFAULT_OUTPUT;
  const outputPath = path.isAbsolute(outputPathInput)
    ? outputPathInput
    : path.resolve(process.cwd(), outputPathInput);

  const client = new ToriiClient(baseUrl, {
    apiToken,
    authToken,
    timeoutMs,
  });

  for (let index = 0; index < samples; index += 1) {
    const snapshot = await appendSumeragiTelemetrySnapshot(client, outputPath);
    process.stdout.write(
      `[telemetry] captured ${index + 1}/${samples} at ${snapshot.capturedAtIso}\n`,
    );
    if (index + 1 < samples && intervalMs > 0) {
      await delay(intervalMs);
    }
  }
}

main().catch((error) => {
  process.stderr.write(`capture_sumeragi_telemetry failed: ${error.message}\n`);
  process.exitCode = 1;
});
