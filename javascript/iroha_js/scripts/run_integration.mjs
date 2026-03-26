#!/usr/bin/env node
// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { setTimeout as delay } from "node:timers/promises";
import { parseArgs } from "node:util";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const JS_DIR = path.resolve(SCRIPT_DIR, "..");
const REPO_ROOT = path.resolve(SCRIPT_DIR, "../../..");
const DEFAULT_COMPOSE_FILE = path.join(REPO_ROOT, "defaults", "docker-compose.single.yml");
const DEFAULT_TORII_URL = process.env.IROHA_TORII_INTEGRATION_URL ?? "http://127.0.0.1:8080";
const DEFAULT_SERVICE = process.env.COMPOSE_SERVICE ?? "irohad0";
const DEFAULT_WAIT_SECONDS = Number.parseInt(process.env.JS_TORII_WAIT_SECONDS ?? process.env.WAIT_SECONDS ?? "", 10) || 90;
const DEFAULT_ACCOUNT_ID =
  process.env.IROHA_TORII_INTEGRATION_ACCOUNT_ID ??
  "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ";
const DEFAULT_PRIVATE_KEY_HEX =
  process.env.IROHA_TORII_INTEGRATION_PRIVATE_KEY_HEX ??
  "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53";
const DEFAULT_CHAIN_ID =
  process.env.IROHA_TORII_INTEGRATION_CHAIN_ID ?? "00000000-0000-0000-0000-000000000000";

const DEFAULT_COMPOSE_FILE_ENV = process.env.JS_TORII_COMPOSE_FILE ?? process.env.COMPOSE_FILE;

const { values, positionals } = parseArgs({
  options: {
    "torii-url": { type: "string" },
    "compose-file": { type: "string" },
    service: { type: "string" },
    "wait-seconds": { type: "string" },
    "compose-bin": { type: "string" },
    start: { type: "boolean" },
    "no-start": { type: "boolean" },
    "skip-install": { type: "boolean" },
    "enable-iso": { type: "boolean" },
    "iso-alias": { type: "string" },
    "iso-alias-index": { type: "string" },
    "iso-pacs008": { type: "string" },
    "iso-pacs009": { type: "string" },
  },
  allowPositionals: true,
});

const startDefault = parseBooleanEnv(process.env.JS_TORII_START ?? "1");
const shouldStart =
  values.start ??
  (values["no-start"] !== undefined ? !values["no-start"] : startDefault);
const toriiUrl = values["torii-url"] ?? DEFAULT_TORII_URL;
const composeFileInput = values["compose-file"] ?? DEFAULT_COMPOSE_FILE_ENV ?? DEFAULT_COMPOSE_FILE;
const composeFile = path.isAbsolute(composeFileInput)
  ? composeFileInput
  : path.resolve(REPO_ROOT, composeFileInput);
const composeService = values.service ?? DEFAULT_SERVICE;
const waitSeconds = values["wait-seconds"]
  ? Number.parseInt(values["wait-seconds"], 10)
  : DEFAULT_WAIT_SECONDS;
const composeBin = values["compose-bin"] ?? process.env.JS_TORII_COMPOSE_BIN ?? process.env.COMPOSE_BIN;
const skipInstall = values["skip-install"] ?? parseBooleanEnv(process.env.JS_TORII_SKIP_INSTALL ?? "0");
const isoCliOptions = {
  enabled: values["enable-iso"],
  alias: values["iso-alias"],
  aliasIndex: values["iso-alias-index"],
  pacs008: values["iso-pacs008"],
  pacs009: values["iso-pacs009"],
};
const isoEnvDefault = parseBooleanEnv(process.env.JS_TORII_ENABLE_ISO ?? "0");

const DEFAULT_TEST_TARGET = "test/integrationTorii.test.js";
const testArgs = normalizeTestArgs(positionals.length === 0 ? [DEFAULT_TEST_TARGET] : positionals);

async function main() {
  if (shouldStart && !existsSync(composeFile)) {
    throw new Error(`compose file not found: ${composeFile}`);
  }

  const composeCommand = shouldStart ? await detectComposeCommand(composeBin) : null;
  let composeRunning = false;

  try {
    if (!skipInstall) {
      await runProcess("npm", ["ci", "--no-audit", "--prefer-offline"], { cwd: JS_DIR });
    }

    await runProcess("npm", ["run", "build:native"], { cwd: JS_DIR });

    if (shouldStart && composeCommand) {
      await runCompose(composeCommand, ["-f", composeFile, "up", "-d", composeService]);
      composeRunning = true;
    }

    await waitForTorii(toriiUrl, waitSeconds);

    const testEnv = {
      ...process.env,
    };
    const isoJsonOverrides = await loadIsoJsonOverrides(isoCliOptions);
    const isoEnabled = isoCliOptions.enabled ?? isoEnvDefault;
    if (!testEnv.IROHA_TORII_INTEGRATION_URL) {
      testEnv.IROHA_TORII_INTEGRATION_URL = toriiUrl;
    }
    if (!testEnv.IROHA_TORII_INTEGRATION_MUTATE) {
      testEnv.IROHA_TORII_INTEGRATION_MUTATE = "1";
    }
    if (!testEnv.IROHA_TORII_INTEGRATION_CHAIN_ID) {
      testEnv.IROHA_TORII_INTEGRATION_CHAIN_ID = DEFAULT_CHAIN_ID;
    }
    if (!testEnv.IROHA_TORII_INTEGRATION_ACCOUNT_ID) {
      testEnv.IROHA_TORII_INTEGRATION_ACCOUNT_ID = DEFAULT_ACCOUNT_ID;
    }
    if (!testEnv.IROHA_TORII_INTEGRATION_PRIVATE_KEY_HEX) {
      testEnv.IROHA_TORII_INTEGRATION_PRIVATE_KEY_HEX = DEFAULT_PRIVATE_KEY_HEX;
    }
    if (isoEnabled && !testEnv.IROHA_TORII_INTEGRATION_ISO_ENABLED) {
      testEnv.IROHA_TORII_INTEGRATION_ISO_ENABLED = "1";
    }
    const isoAlias = normalizeCliString(isoCliOptions.alias);
    if (isoAlias && !testEnv.IROHA_TORII_INTEGRATION_ISO_ALIAS) {
      testEnv.IROHA_TORII_INTEGRATION_ISO_ALIAS = isoAlias;
    }
    const isoAliasIndex = normalizeCliString(isoCliOptions.aliasIndex);
    if (isoAliasIndex && !testEnv.IROHA_TORII_INTEGRATION_ISO_ALIAS_INDEX) {
      testEnv.IROHA_TORII_INTEGRATION_ISO_ALIAS_INDEX = isoAliasIndex;
    }
    if (isoJsonOverrides.pacs008 && !testEnv.IROHA_TORII_INTEGRATION_ISO_PACS008) {
      testEnv.IROHA_TORII_INTEGRATION_ISO_PACS008 = isoJsonOverrides.pacs008;
    }
    if (isoJsonOverrides.pacs009 && !testEnv.IROHA_TORII_INTEGRATION_ISO_PACS009) {
      testEnv.IROHA_TORII_INTEGRATION_ISO_PACS009 = isoJsonOverrides.pacs009;
    }

    await runProcess("node", ["--test", ...testArgs], {
      cwd: JS_DIR,
      env: testEnv,
    });
  } finally {
    if (composeRunning && composeCommand) {
      try {
        await runCompose(composeCommand, ["-f", composeFile, "down", "--remove-orphans"]);
      } catch (error) {
        console.warn(`[integration] failed to stop compose stack: ${error}`);
      }
    }
  }
}

function parseBooleanEnv(value) {
  if (!value) {
    return false;
  }
  const normalized = value.trim().toLowerCase();
  return normalized !== "0" && normalized !== "false";
}

function normalizeTestArgs(positionals) {
  if (positionals.length === 0) {
    return [DEFAULT_TEST_TARGET];
  }
  if (positionals[0] === "--") {
    return positionals.slice(1);
  }
  return positionals;
}

async function detectComposeCommand(explicit) {
  const candidates = [];
  if (explicit) {
    candidates.push(explicit);
  }
  candidates.push("docker compose", "docker-compose");

  for (const candidate of candidates) {
    const parts = splitCommand(candidate);
    if (parts.length === 0) {
      continue;
    }
    const [bin, ...rest] = parts;
    try {
      await runProcess(bin, [...rest, "version"], {
        stdio: "ignore",
      });
      return parts;
    } catch {
      // try next candidate
    }
  }

  throw new Error(
    "docker compose is required; install Docker or set JS_TORII_COMPOSE_BIN to the desired command",
  );
}

function splitCommand(value) {
  return value
    .split(/\s+/)
    .map((part) => part.trim())
    .filter(Boolean);
}

async function runCompose(composeCommand, args) {
  const [bin, ...rest] = composeCommand;
  await runProcess(bin, [...rest, ...args], { cwd: REPO_ROOT });
}

async function waitForTorii(url, waitSeconds) {
  const deadline = Date.now() + waitSeconds * 1000;
  const statusUrl = new URL("/status", url).toString();

  while (Date.now() <= deadline) {
    try {
      const response = await fetch(statusUrl, { headers: { Accept: "application/json" } });
      if (response.ok) {
        return;
      }
    } catch {
      // ignore and retry
    }
    await delay(2000);
  }

  throw new Error(`Timed out waiting for Torii at ${statusUrl}`);
}

function runProcess(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      stdio: "inherit",
      ...options,
    });
    child.on("error", reject);
    child.on("exit", (code, signal) => {
      if (code === 0) {
        resolve();
      } else {
        reject(
          new Error(
            `${command} ${args.join(" ")} exited with ${
              signal ? `signal ${signal}` : `code ${code}`
            }`,
          ),
        );
      }
    });
  });
}

async function loadIsoJsonOverrides(options) {
  const overrides = {};
  if (options.pacs008) {
    overrides.pacs008 = await resolveIsoJsonInput(options.pacs008, "pacs.008 override");
  }
  if (options.pacs009) {
    overrides.pacs009 = await resolveIsoJsonInput(options.pacs009, "pacs.009 override");
  }
  return overrides;
}

async function resolveIsoJsonInput(input, label) {
  const normalized = normalizeCliString(input);
  if (!normalized) {
    return null;
  }
  const candidate = path.isAbsolute(normalized)
    ? normalized
    : path.resolve(process.cwd(), normalized);
  if (existsSync(candidate)) {
    const contents = await readFile(candidate, "utf8");
    JSON.parse(contents);
    return contents;
  }
  try {
    JSON.parse(normalized);
  } catch (error) {
    throw new Error(
      `invalid JSON for ${label}: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  return normalized;
}

function normalizeCliString(value) {
  if (typeof value !== "string") {
    return null;
  }
  const trimmed = value.trim();
  return trimmed.length === 0 ? null : trimmed;
}

main().catch((error) => {
  console.error(`[integration] ${error.message}`);
  process.exitCode = 1;
});
