#!/usr/bin/env node
// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import { spawn } from "node:child_process";
import { readdirSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const SCRIPT_DIRECTORY = path.dirname(fileURLToPath(import.meta.url));
const SDK_DIRECTORY = path.resolve(SCRIPT_DIRECTORY, "..");
const TEST_DIRECTORY = path.join(SDK_DIRECTORY, "test");

const PROFILE_FILES = Object.freeze({
  heavy: Object.freeze(["sorafsChunker.oneGib.test.js"]),
  "native-provenance": Object.freeze(["nativeBuildProvenance.test.js"]),
  "sorafs-native": Object.freeze([
    "cancelAssetLockV1.test.js",
    "sorafsAppealFinanceValidation.test.js",
    "sorafsFixtureBundleValidation.test.js",
    "sorafsOrchestrator.parity.test.js",
    "sorafsPdpValidation.test.js",
  ]),
});

const UNIT_EXCLUSIONS = new Set([
  "integrationTorii.test.js",
  ...PROFILE_FILES.heavy,
]);

function discoverTests(directory, prefix = "") {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const relativePath = prefix ? `${prefix}/${entry.name}` : entry.name;
    const absolutePath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...discoverTests(absolutePath, relativePath));
    } else if (
      entry.isFile() &&
      (entry.name.endsWith(".test.js") || entry.name.endsWith(".test.mjs"))
    ) {
      files.push(relativePath);
    }
  }
  return files.sort();
}

function selectedTests(profile) {
  if (profile === "unit") {
    return discoverTests(TEST_DIRECTORY).filter(
      (relativePath) => !UNIT_EXCLUSIONS.has(relativePath),
    );
  }
  const files = PROFILE_FILES[profile];
  if (!files) {
    throw new Error(
      `unknown JavaScript test profile ${JSON.stringify(profile)}; expected unit, native-provenance, sorafs-native, or heavy`,
    );
  }
  return [...files];
}

function relayAndInspect(stream, destination, state) {
  let pending = "";
  stream.setEncoding("utf8");
  stream.on("data", (chunk) => {
    destination.write(chunk);
    pending += chunk;
    const lines = pending.split(/\r?\n/u);
    pending = lines.pop() ?? "";
    for (const line of lines) {
      if (/# (?:SKIP|TODO)(?:\s|$)/u.test(line)) {
        state.unsatisfied.push(line.trim());
      }
    }
  });
  stream.on("end", () => {
    if (/# (?:SKIP|TODO)(?:\s|$)/u.test(pending)) {
      state.unsatisfied.push(pending.trim());
    }
  });
}

async function run(profile) {
  const files = selectedTests(profile);
  if (files.length === 0) {
    throw new Error(`JavaScript test profile ${profile} selected no tests`);
  }

  const state = { unsatisfied: [] };
  const child = spawn(
    process.execPath,
    ["--test", "--test-reporter=tap", ...files.map((file) => path.join("test", file))],
    {
      cwd: SDK_DIRECTORY,
      env: process.env,
      stdio: ["inherit", "pipe", "pipe"],
    },
  );
  relayAndInspect(child.stdout, process.stdout, state);
  relayAndInspect(child.stderr, process.stderr, state);

  const outcome = await new Promise((resolve, reject) => {
    child.once("error", reject);
    child.once("exit", (code, signal) => resolve({ code, signal }));
  });
  if (outcome.signal) {
    throw new Error(`JavaScript test profile ${profile} terminated by ${outcome.signal}`);
  }
  if (outcome.code !== 0) {
    throw new Error(`JavaScript test profile ${profile} exited with code ${outcome.code}`);
  }
  if (state.unsatisfied.length !== 0) {
    throw new Error(
      `JavaScript test profile ${profile} reported skipped or todo tests:\n${state.unsatisfied.join("\n")}`,
    );
  }
}

const profile = process.argv[2] ?? "unit";
run(profile).catch((error) => {
  console.error(`[javascript-tests] ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
});
